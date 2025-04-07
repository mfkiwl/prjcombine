use clap::Parser;
use prjcombine_re_toolchain::Toolchain;
use prjcombine_re_xilinx_vivado_dump::parts::get_parts;
use std::{error::Error, path::PathBuf};

#[derive(Debug, Parser)]
#[command(
    name = "dump_vivado_parts",
    about = "Dump Vivado part geometry into rawdump files."
)]
struct Args {
    toolchain: PathBuf,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let tc = Toolchain::from_file(&args.toolchain)?;
    let parts = get_parts(&tc)?;
    for part in parts {
        println!("{part:?}");
    }
    Ok(())
}
