use clap::Parser;
use prjcombine_re_xilinx_rawdump::Part;
use simple_error::bail;
use std::error::Error;
use std::path::PathBuf;
use std::sync::Mutex;
use std_semaphore::Semaphore;

mod db;
mod spartan6;
mod ultrascale;
mod versal;
mod virtex;
mod virtex2;
mod virtex4;
mod virtex5;
mod virtex6;
mod virtex7;
mod xc4000;
mod xc5200;

#[derive(Debug, Parser)]
#[command(
    name = "prjcombine_xilinx_rd2geom",
    about = "Extract geometry information from rawdumps."
)]
struct Args {
    dst: PathBuf,
    files: Vec<PathBuf>,
    #[arg(long)]
    no_verify: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    if args.files.is_empty() {
        bail!("no files given");
    }
    let builder = Mutex::new(db::DbBuilder::new());
    let rb = &builder;
    let sema = Semaphore::new(std::thread::available_parallelism().unwrap().get() as isize);
    let verify = !args.no_verify;
    std::thread::scope(|s| {
        for fname in args.files {
            let guard = sema.access();
            let tname = fname.file_stem().unwrap().to_str().unwrap();
            std::thread::Builder::new()
                .name(tname.to_string())
                .spawn_scoped(s, move || {
                    let rd = Part::from_file(fname).unwrap();
                    println!("INGEST {} {:?}", rd.part, rd.source);
                    let pre = match &rd.family[..] {
                        "xc4000e" | "xc4000ex" | "xc4000xla" | "xc4000xv" | "spartanxl" => {
                            xc4000::ingest(&rd, verify)
                        }
                        "xc5200" => xc5200::ingest(&rd, verify),
                        "virtex" | "virtexe" => virtex::ingest(&rd, verify),
                        "virtex2" | "virtex2p" | "spartan3" | "spartan3e" | "spartan3a"
                        | "spartan3adsp" | "fpgacore" => virtex2::ingest(&rd, verify),
                        "spartan6" => spartan6::ingest(&rd, verify),
                        "virtex4" => virtex4::ingest(&rd, verify),
                        "virtex5" => virtex5::ingest(&rd, verify),
                        "virtex6" => virtex6::ingest(&rd, verify),
                        "virtex7" => virtex7::ingest(&rd, verify),
                        "ultrascale" | "ultrascaleplus" => ultrascale::ingest(&rd, verify),
                        "versal" => versal::ingest(&rd, verify),
                        _ => panic!("unknown family {}", rd.family),
                    };
                    let mut builder = rb.lock().unwrap();
                    builder.ingest(pre);
                    std::mem::drop(guard);
                })
                .unwrap();
        }
    });
    let db = builder.into_inner().unwrap().finish();
    db.to_file(args.dst)?;
    Ok(())
}
