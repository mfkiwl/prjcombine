use arrayref::array_ref;
use prjcombine_re_toolchain::Toolchain;
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::fs::File;
use std::io::{self, Write};
use std::mem;

mod parser;

#[derive(Debug)]
pub struct Design {
    pub name: String,
    pub part: String,
    pub version: String,
    pub cfg: Config,
    pub instances: Vec<Instance>,
    pub nets: Vec<Net>,
}

#[derive(Debug)]
pub struct Instance {
    pub name: String,
    pub kind: String,
    pub placement: Placement,
    pub cfg: Config,
}

#[derive(Debug)]
pub enum Placement {
    Placed { tile: String, site: String },
    Unplaced,
    Bonded,
    Unbonded,
}

type Config = Vec<Vec<String>>;

#[derive(Debug)]
pub struct Net {
    pub name: String,
    pub typ: NetType,
    pub inpins: Vec<NetPin>,
    pub outpins: Vec<NetPin>,
    pub pips: Vec<NetPip>,
    pub cfg: Config,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum NetType {
    Plain,
    Gnd,
    Vcc,
}

#[derive(Debug)]
pub struct NetPin {
    pub inst_name: String,
    pub pin: String,
}

#[derive(Debug)]
pub struct NetPip {
    pub tile: String,
    pub wire_from: String,
    pub wire_to: String,
    pub dir: PipDirection,
}

#[derive(Debug)]
pub enum PipDirection {
    Unbuf,
    BiUniBuf,
    BiBuf,
    UniBuf,
}

struct FmtString<'a>(&'a str);

fn fmt_string(s: &str) -> FmtString<'_> {
    FmtString(s)
}

impl Display for FmtString<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "\"")?;
        for c in self.0.chars() {
            match c {
                '\\' | '"' => write!(f, "\\{c}")?,
                _ => write!(f, "{c}")?,
            }
        }
        write!(f, "\"")?;
        Ok(())
    }
}

struct FmtCfg<'a>(&'a Config);

fn fmt_cfg(c: &Config) -> FmtCfg<'_> {
    FmtCfg(c)
}

impl Display for FmtCfg<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(f, "\"")?;
        let mut first = true;
        for chunk in self.0 {
            if !first {
                writeln!(f)?;
            }
            write!(f, "  ")?;
            first = false;
            let mut first_part = true;
            for part in chunk {
                if !first_part {
                    write!(f, ":")?;
                }
                first_part = false;
                for c in part.chars() {
                    match c {
                        '\\' | '"' | ':' | ' ' => write!(f, "\\{c}")?,
                        _ => write!(f, "{c}")?,
                    }
                }
            }
        }
        write!(f, "\"")?;
        Ok(())
    }
}

#[derive(Debug)]
pub enum ParseErrorKind {
    UnclosedString,
    ExpectedWord,
    ExpectedString,
    ExpectedDesign,
    ExpectedCfg,
    ExpectedCommaSemi,
    ExpectedComma,
    ExpectedSemi,
    ExpectedTop,
    ExpectedPlacement,
    ExpectedNetItem,
    ExpectedPipDirection,
}

#[derive(Debug)]
pub struct ParseError {
    pub kind: ParseErrorKind,
    pub line: u32,
}

impl Display for ParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let desc = match self.kind {
            ParseErrorKind::UnclosedString => "unclosed string",
            ParseErrorKind::ExpectedWord => "expected a word",
            ParseErrorKind::ExpectedString => "expected a string",
            ParseErrorKind::ExpectedDesign => "expected `design`",
            ParseErrorKind::ExpectedCfg => "expected `cfg`",
            ParseErrorKind::ExpectedCommaSemi => "expected `,` or `;`",
            ParseErrorKind::ExpectedComma => "expected `,`",
            ParseErrorKind::ExpectedSemi => "expected `;`",
            ParseErrorKind::ExpectedTop => "expected `instance` or `net``",
            ParseErrorKind::ExpectedPlacement => "expected `placed` or `unplaced`",
            ParseErrorKind::ExpectedNetItem => "expected `inpin`, `outpin`, or `pip`",
            ParseErrorKind::ExpectedPipDirection => "expected `==`, `=>`, `=-`, or `->`",
        };
        write!(f, "parse error in line {}: {}", self.line, desc)
    }
}

impl Error for ParseError {}

impl Design {
    pub fn write(&self, f: &mut dyn Write) -> io::Result<()> {
        write!(f, "design {} {}", fmt_string(&self.name), self.part,)?;
        if !self.version.is_empty() {
            write!(f, " {}", self.version)?;
        }
        if !self.cfg.is_empty() {
            write!(f, ", cfg {}", fmt_cfg(&self.cfg))?;
        }
        write!(f, ";\n\n")?;

        for inst in &self.instances {
            write!(
                f,
                "inst {} {}, ",
                fmt_string(&inst.name),
                fmt_string(&inst.kind)
            )?;
            match &inst.placement {
                Placement::Placed { tile, site } => write!(f, "placed \"{tile}\" \"{site}\"")?,
                Placement::Unplaced => write!(f, "unplaced")?,
                Placement::Bonded => write!(f, "unplaced bonded")?,
                Placement::Unbonded => write!(f, "unplaced unbonded")?,
            }
            write!(f, ", cfg {};\n\n", fmt_cfg(&inst.cfg))?;
        }

        for net in &self.nets {
            write!(f, "net {}", fmt_string(&net.name))?;
            match net.typ {
                NetType::Plain => (),
                NetType::Gnd => write!(f, " gnd")?,
                NetType::Vcc => write!(f, " vcc")?,
            }
            if !net.cfg.is_empty() {
                write!(f, ", cfg {}", fmt_cfg(&net.cfg))?;
            }
            writeln!(f, ",")?;
            for pin in &net.outpins {
                writeln!(f, "  outpin {} {},", fmt_string(&pin.inst_name), pin.pin)?;
            }
            for pin in &net.inpins {
                writeln!(f, "  inpin {} {},", fmt_string(&pin.inst_name), pin.pin)?;
            }
            for pip in &net.pips {
                let dir = match pip.dir {
                    PipDirection::Unbuf => "==",
                    PipDirection::BiBuf => "=-",
                    PipDirection::BiUniBuf => "=>",
                    PipDirection::UniBuf => "->",
                };
                writeln!(
                    f,
                    " pip \"{}\" \"{}\" {} \"{}\",",
                    pip.tile, pip.wire_from, dir, pip.wire_to
                )?;
            }
            write!(f, ";\n\n")?;
        }
        Ok(())
    }

    pub fn parse(s: &str) -> Result<Self, ParseError> {
        parser::parse(s)
    }
}

pub fn parse_lut(sz: u8, val: &str) -> Option<u64> {
    let rval = match sz {
        4 => val.strip_prefix("D=")?,
        5 => val.strip_prefix("O5=")?,
        6 => val.strip_prefix("O6=")?,
        _ => panic!("invalid sz"),
    };
    let mask = match sz {
        4 => 0xffff,
        5 => 0xffffffff,
        6 => 0xffffffffffffffff,
        _ => panic!("invalid sz"),
    };
    if let Some(rv) = rval.strip_prefix("0x") {
        u64::from_str_radix(rv, 16).ok()
    } else {
        #[derive(Eq, PartialEq, Copy, Clone, Debug)]
        enum StackEntry {
            Val(u64),
            And,
            Or,
            Xor,
            Not,
            Par,
        }
        let mut stack = Vec::new();
        let mut ch = rval.chars();
        loop {
            let c = ch.next();
            while let &[.., StackEntry::Not, StackEntry::Val(v)] = &stack[..] {
                stack.pop();
                stack.pop();
                stack.push(StackEntry::Val(!v));
            }
            while let &[.., StackEntry::Val(v1), StackEntry::And, StackEntry::Val(v2)] = &stack[..]
            {
                stack.pop();
                stack.pop();
                stack.pop();
                stack.push(StackEntry::Val(v1 & v2));
            }
            if c == Some('*') {
                stack.push(StackEntry::And);
                continue;
            }
            while let &[.., StackEntry::Val(v1), StackEntry::Xor, StackEntry::Val(v2)] = &stack[..]
            {
                stack.pop();
                stack.pop();
                stack.pop();
                stack.push(StackEntry::Val(v1 ^ v2));
            }
            if c == Some('@') {
                stack.push(StackEntry::Xor);
                continue;
            }
            while let &[.., StackEntry::Val(v1), StackEntry::Or, StackEntry::Val(v2)] = &stack[..] {
                stack.pop();
                stack.pop();
                stack.pop();
                stack.push(StackEntry::Val(v1 | v2));
            }
            if c.is_none() {
                break;
            }
            match c.unwrap() {
                '(' => stack.push(StackEntry::Par),
                '0' => stack.push(StackEntry::Val(0)),
                '1' => stack.push(StackEntry::Val(0xffffffffffffffff)),
                'A' => {
                    stack.push(StackEntry::Val(match ch.next()? {
                        '1' => 0xaaaaaaaaaaaaaaaa,
                        '2' => 0xcccccccccccccccc,
                        '3' => 0xf0f0f0f0f0f0f0f0,
                        '4' => 0xff00ff00ff00ff00,
                        '5' => 0xffff0000ffff0000,
                        '6' => 0xffffffff00000000,
                        _ => return None,
                    }));
                }
                '+' => stack.push(StackEntry::Or),
                '~' => stack.push(StackEntry::Not),
                ')' => {
                    if let &[.., StackEntry::Par, StackEntry::Val(v)] = &stack[..] {
                        stack.pop();
                        stack.pop();
                        stack.push(StackEntry::Val(v));
                    } else {
                        return None;
                    }
                }
                _ => return None,
            }
        }
        if stack.len() == 1 {
            if let StackEntry::Val(r) = stack[0] {
                return Some(r & mask);
            }
        }
        None
    }
}

pub struct Pcf {
    pub vccaux: Option<String>,
    pub internal_vref: HashMap<u32, u32>,
    pub dci_cascade: HashMap<u32, u32>,
    pub vccosensemode: HashMap<u32, String>,
}

pub fn run_bitgen(
    tc: &Toolchain,
    design: &Design,
    gopts: &HashMap<String, String>,
    pcf: &Pcf,
    altvr: bool,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let dir = tempfile::Builder::new().prefix("xdl_bitgen").tempdir()?;
    let mut xdl_file = File::create(dir.path().join("meow.xdl"))?;
    design.write(&mut xdl_file)?;
    if design.nets.is_empty() && design.version.is_empty() {
        writeln!(xdl_file, "net \"meow\";")?;
    }
    std::mem::drop(xdl_file);
    let mut cmd = tc.command("xdl");
    cmd.current_dir(dir.path());
    cmd.env("XIL_TEST_ARCS", "1");
    cmd.env("XIL_DRM_EXCLUDE_ARCS", "1");
    if altvr {
        cmd.env("XIL_VIRTEX2_PKG_USEALT", "1");
    }
    cmd.arg("-xdl2ncd");
    cmd.arg("-force");
    cmd.arg("meow.xdl");
    let status = cmd.output()?;
    if !status.status.success() {
        let _ = std::io::stderr().write_all(&status.stdout);
        let _ = std::io::stderr().write_all(&status.stderr);
        mem::forget(dir);
        panic!("non-zero xdl exit status");
    }
    let mut pcf_file = File::create(dir.path().join("meow.pcf"))?;
    writeln!(pcf_file)?;
    if let Some(ref val) = pcf.vccaux {
        writeln!(pcf_file, "CONFIG VCCAUX=\"{val}\";")?;
    }
    for (&bank, &vref) in &pcf.internal_vref {
        writeln!(
            pcf_file,
            "CONFIG INTERNAL_VREF_BANK{bank}={h}.{l:03};",
            h = vref / 1000,
            l = vref % 1000
        )?;
    }
    let mut dci_groups: HashMap<u32, Vec<u32>> = HashMap::new();
    for (&bank, &tgt) in &pcf.dci_cascade {
        dci_groups.entry(tgt).or_default().push(bank);
    }
    for (src, others) in dci_groups {
        write!(pcf_file, "CONFIG DCI_CASCADE = \"{src}")?;
        for val in others {
            write!(pcf_file, ", {val}")?;
        }
        writeln!(pcf_file, "\";")?;
    }
    for (&bank, mode) in &pcf.vccosensemode {
        writeln!(pcf_file, "CONFIG VCCOSENSEMODE{bank}={mode};",)?;
    }
    std::mem::drop(pcf_file);
    let mut cmd = tc.command("bitgen");
    cmd.current_dir(dir.path());
    cmd.env("XIL_TEST_ARCS", "1");
    cmd.env("XIL_DRM_EXCLUDE_ARCS", "1");
    if altvr {
        cmd.env("XIL_VIRTEX2_PKG_USEALT", "1");
    }
    cmd.arg("-d");
    for (k, v) in gopts {
        cmd.arg("-g");
        if v.is_empty() {
            cmd.arg(k);
        } else {
            cmd.arg(format!("{k}:{v}"));
        }
    }
    cmd.arg("meow.ncd");
    cmd.arg("meow.bit");
    cmd.arg("meow.pcf");
    let status = cmd.output()?;
    if !status.status.success() {
        let _ = std::io::stderr().write_all(&status.stdout);
        let _ = std::io::stderr().write_all(&status.stderr);
        mem::forget(dir);
        panic!("non-zero bitgen exit status");
    }
    let mut bitdata = std::fs::read(dir.path().join("meow.bit"))?;
    assert_eq!(
        bitdata[..13],
        [0x00, 0x09, 0x0f, 0xf0, 0x0f, 0xf0, 0x0f, 0xf0, 0x0f, 0xf0, 0x00, 0x00, 0x01]
    );
    let mut pos = 13;
    for l in [b'a', b'b', b'c', b'd'] {
        assert_eq!(bitdata[pos], l);
        pos += 1;
        let len = u16::from_be_bytes(*array_ref!(bitdata, pos, 2)) as usize;
        pos += 2;
        pos += len;
    }
    assert_eq!(bitdata[pos], b'e');
    pos += 1;
    let len = u32::from_be_bytes(*array_ref!(bitdata, pos, 4)) as usize;
    pos += 4;
    assert_eq!(pos + len, bitdata.len());
    bitdata.drain(0..pos);
    Ok(bitdata)
}
