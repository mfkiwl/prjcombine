use crate::{Cell, Delay, Edge, IoPath, Period, Port, Sdf, SetupHold, Width};

#[derive(Debug, PartialEq)]
enum Token {
    LParen,
    RParen,
    Id(String),
    String(String),
    Float(f64),
    Integer(i64),
    Slash,
    Colon,
}

struct Lexer<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Lexer<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }
}

impl Iterator for Lexer<'_> {
    type Item = Token;

    fn next(&mut self) -> Option<Token> {
        if let Some(x) = self.input[self.pos..].find(|x: char| !x.is_whitespace()) {
            self.pos += x;
        } else {
            self.pos = self.input.len();
        }
        match self.input[self.pos..].chars().next() {
            None => None,
            Some('(') => {
                self.pos += 1;
                Some(Token::LParen)
            }
            Some(')') => {
                self.pos += 1;
                Some(Token::RParen)
            }
            Some(':') => {
                self.pos += 1;
                Some(Token::Colon)
            }
            Some('/') => {
                self.pos += 1;
                Some(Token::Slash)
            }
            Some('"') => {
                self.pos += 1;
                let mut s = String::new();
                let mut ch = self.input[self.pos..].char_indices();
                loop {
                    match ch.next() {
                        None => panic!("unclosed string"),
                        Some((_, '\\')) => {
                            let (_, c) = ch.next().unwrap();
                            s.push(c);
                        }
                        Some((i, '"')) => {
                            self.pos += i + 1;
                            break;
                        }
                        Some((_, c)) => {
                            s.push(c);
                        }
                    }
                }
                Some(Token::String(s))
            }
            Some(x) if x.is_ascii_digit() || x == '-' => {
                let n = self.input[self.pos..]
                    .find(|x: char| !matches!(x, '0'..='9' | '.' | 'e' | 'E' | '-' | '+'));
                let epos = n.map(|x| self.pos + x).unwrap_or(self.input.len());
                let num = &self.input[self.pos..epos];
                self.pos = epos;
                Some(if num.contains(|x: char| !x.is_ascii_digit() && x != '-') {
                    Token::Float(num.parse().unwrap())
                } else {
                    Token::Integer(num.parse().unwrap())
                })
            }
            Some(x) if x.is_ascii_alphabetic() || x == '_' || x == '$' => {
                let mut s = String::new();
                let mut ch = self.input[self.pos..].char_indices();
                loop {
                    match ch.next() {
                        None => {
                            self.pos = self.input.len();
                            break;
                        }
                        Some((_, '\\')) => {
                            let (_, c) = ch.next().unwrap();
                            s.push(c);
                        }
                        Some((_, c)) if c.is_ascii_alphanumeric() || c == '_' || c == '$' => {
                            s.push(c);
                        }
                        Some((i, _)) => {
                            self.pos += i;
                            break;
                        }
                    }
                }
                Some(Token::Id(s))
            }
            Some(x) => panic!("weird char '{x}'"),
        }
    }
}

struct Parser<'a> {
    sdf: Sdf,
    lexer: Lexer<'a>,
}

impl Parser<'_> {
    fn parse(&mut self) {
        self.eat_lp();
        assert_eq!(self.get_id(), "DELAYFILE");
        loop {
            match self.lexer.next() {
                Some(Token::LParen) => (),
                Some(Token::RParen) => {
                    assert_eq!(self.lexer.next(), None);
                    return;
                }
                _ => panic!("weird top-level token"),
            }
            let id = self.get_id();
            match &*id {
                "SDFVERSION" => {
                    assert!(self.sdf.sdfversion.is_none());
                    self.sdf.sdfversion = Some(self.get_string());
                    self.eat_rp();
                }
                "DESIGN" => {
                    assert!(self.sdf.design.is_none());
                    self.sdf.design = Some(self.get_string());
                    self.eat_rp();
                }
                "DATE" => {
                    assert!(self.sdf.date.is_none());
                    self.sdf.date = Some(self.get_string());
                    self.eat_rp();
                }
                "VENDOR" => {
                    assert!(self.sdf.vendor.is_none());
                    self.sdf.vendor = Some(self.get_string());
                    self.eat_rp();
                }
                "PROGRAM" => {
                    assert!(self.sdf.program.is_none());
                    self.sdf.program = Some(self.get_string());
                    self.eat_rp();
                }
                "VERSION" => {
                    assert!(self.sdf.version.is_none());
                    self.sdf.version = Some(self.get_string());
                    self.eat_rp();
                }
                "DIVIDER" => {
                    assert_eq!(self.lexer.next(), Some(Token::Slash));
                    self.eat_rp();
                }
                "TIMESCALE" => {
                    let i = self.get_int();
                    let u = self.get_id();
                    assert!(self.sdf.timescale.is_none());
                    let its = match i {
                        1 => 0,
                        10 => 1,
                        100 => 2,
                        _ => panic!("weird timescale"),
                    };
                    let uts = match &*u {
                        "fs" => 0,
                        "ps" => 3,
                        "ns" => 6,
                        "us" => 9,
                        "ms" => 12,
                        "s" => 15,
                        _ => panic!("weird timescale"),
                    };
                    self.sdf.timescale = Some(its + uts);
                    self.eat_rp();
                }
                "CELL" => self.parse_cell(),

                _ => panic!("weird top-level item {id}"),
            }
        }
    }

    fn parse_cell(&mut self) {
        self.eat_lp();
        self.eat_id("CELLTYPE");
        let typ = self.get_string();
        self.eat_rp();
        self.eat_lp();
        self.eat_id("INSTANCE");
        let name = self.get_id();
        self.eat_rp();
        let mut cell = Cell {
            typ,
            iopath: vec![],
            ports: vec![],
            setuphold: vec![],
            period: vec![],
            width: vec![],
        };
        loop {
            match self.lexer.next() {
                Some(Token::LParen) => (),
                Some(Token::RParen) => {
                    assert!(self.sdf.cells.insert(name, cell).is_none());
                    return;
                }
                _ => panic!("weird cell item"),
            }
            let id = self.get_id();
            match &*id {
                "DELAY" => self.parse_delay(&mut cell),
                "TIMINGCHECK" => self.parse_timingcheck(&mut cell),
                _ => panic!("weird cell item {id}"),
            }
        }
    }

    fn parse_delay(&mut self, cell: &mut Cell) {
        loop {
            match self.lexer.next() {
                Some(Token::LParen) => (),
                Some(Token::RParen) => return,
                _ => panic!("weird delay item"),
            }
            let id = self.get_id();
            match &*id {
                "ABSOLUTE" => self.parse_absolute(cell),
                _ => panic!("weird delay item {id}"),
            }
        }
    }

    fn parse_absolute(&mut self, cell: &mut Cell) {
        loop {
            match self.lexer.next() {
                Some(Token::LParen) => (),
                Some(Token::RParen) => return,
                _ => panic!("weird absolute item"),
            }
            let id = self.get_id();
            match &*id {
                "IOPATH" => self.parse_iopath(cell),
                "PORT" => self.parse_port(cell),
                _ => panic!("weird absolute item {id}"),
            }
        }
    }

    fn parse_iopath(&mut self, cell: &mut Cell) {
        let port_from = self.get_id();
        let port_to = self.get_id();
        let del_rise = self.get_delay();
        let del_fall = self.get_delay();
        cell.iopath.push(IoPath {
            port_from,
            port_to,
            del_rise,
            del_fall,
        });
        self.eat_rp();
    }

    fn parse_port(&mut self, cell: &mut Cell) {
        let port = self.get_id();
        let del_rise = self.get_delay();
        let del_fall = self.get_delay();
        cell.ports.push(Port {
            port,
            del_rise,
            del_fall,
        });
        self.eat_rp();
    }

    fn parse_timingcheck(&mut self, cell: &mut Cell) {
        loop {
            match self.lexer.next() {
                Some(Token::LParen) => (),
                Some(Token::RParen) => return,
                _ => panic!("weird timingcheck item"),
            }
            let id = self.get_id();
            match &*id {
                "SETUPHOLD" => self.parse_setuphold(cell),
                "PERIOD" => self.parse_period(cell),
                "WIDTH" => self.parse_width(cell),
                _ => panic!("weird timingcheck item {id}"),
            }
        }
    }

    fn parse_setuphold(&mut self, cell: &mut Cell) {
        let edge_d = self.get_edge();
        let edge_c = self.get_edge();
        let setup = self.get_delay();
        let hold = self.get_delay();
        cell.setuphold.push(SetupHold {
            edge_d,
            edge_c,
            setup,
            hold,
        });
        self.eat_rp();
    }

    fn parse_period(&mut self, cell: &mut Cell) {
        let edge = self.get_edge();
        let val = self.get_delay();
        cell.period.push(Period { edge, val });
        self.eat_rp();
    }

    fn parse_width(&mut self, cell: &mut Cell) {
        let edge = self.get_edge();
        let val = self.get_delay();
        cell.width.push(Width { edge, val });
        self.eat_rp();
    }

    fn eat_lp(&mut self) {
        assert_eq!(self.lexer.next(), Some(Token::LParen));
    }

    fn eat_rp(&mut self) {
        assert_eq!(self.lexer.next(), Some(Token::RParen));
    }

    fn eat_colon(&mut self) {
        assert_eq!(self.lexer.next(), Some(Token::Colon));
    }

    fn eat_id(&mut self, exp: &str) {
        assert_eq!(self.get_id(), exp);
    }

    fn get_id(&mut self) -> String {
        if let Some(Token::Id(id)) = self.lexer.next() {
            id
        } else {
            panic!("expected id");
        }
    }

    fn get_string(&mut self) -> String {
        if let Some(Token::String(s)) = self.lexer.next() {
            s
        } else {
            panic!("expected string");
        }
    }

    fn get_int(&mut self) -> i64 {
        if let Some(Token::Integer(i)) = self.lexer.next() {
            i
        } else {
            panic!("expected integer");
        }
    }

    fn get_delay(&mut self) -> Delay {
        self.eat_lp();
        let min = self.get_int();
        match self.lexer.next() {
            Some(Token::RParen) => {
                return Delay {
                    min,
                    typ: min,
                    max: min,
                };
            }
            Some(Token::Colon) => (),
            _ => panic!("weird delay token"),
        }
        let typ = self.get_int();
        self.eat_colon();
        let max = self.get_int();
        self.eat_rp();
        Delay { min, typ, max }
    }

    fn get_edge(&mut self) -> Edge {
        match self.lexer.next() {
            Some(Token::Id(sig)) => Edge::Plain(sig),
            Some(Token::LParen) => {
                let kind = self.get_id();
                let sig = self.get_id();
                self.eat_rp();
                match &*kind {
                    "posedge" => Edge::Posedge(sig),
                    "negedge" => Edge::Negedge(sig),
                    _ => panic!("weird edge kind"),
                }
            }
            _ => panic!("weird edge"),
        }
    }
}

impl Sdf {
    pub fn parse(s: &str) -> Self {
        let mut parser = Parser {
            sdf: Sdf::default(),
            lexer: Lexer::new(s),
        };
        parser.parse();
        parser.sdf
    }
}
