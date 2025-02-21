use clap::Parser;
use itertools::Itertools;
use prjcombine_re_lattice_rawdump::Db;
use std::{error::Error, path::PathBuf};
use unnamed_entity::{EntityBitVec, EntityId};

#[derive(Debug, Parser)]
#[command(name = "lrdprint", about = "Dump Lattice rawdump file.")]
struct Args {
    file: PathBuf,
    part: Option<String>,
    package: Option<String>,
    #[arg(short, long)]
    tiles: bool,
    #[arg(short, long)]
    sites: bool,
    #[arg(short, long)]
    nodes: bool,
    #[arg(short, long)]
    pips: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let lrd = Db::from_file(args.file)?;
    println!("DB {family}", family = lrd.family);
    let mut gids = EntityBitVec::repeat(false, lrd.grids.len());
    for part in &lrd.parts {
        if let Some(ref opart) = args.part {
            if &part.name != opart {
                continue;
            }
        }
        if let Some(ref opkg) = args.package {
            if &part.package != opkg {
                continue;
            }
        }
        print!(
            "PART {arch} {name} {package} GRID {gid} SPEEDS",
            arch = part.arch,
            name = part.name,
            package = part.package,
            gid = part.grid.to_idx()
        );
        for speed in &part.speeds {
            print!(" {speed}");
        }
        println!();
        if args.sites {
            for site in &part.sites {
                if let Some(ref typ) = site.typ {
                    println!("\tSITE {name} {typ}", name = site.name);
                } else {
                    println!("\tSITE {name}", name = site.name);
                }
            }
        }
        gids.set(part.grid, true);
    }
    for (gid, grid) in &lrd.grids {
        if !gids[gid] {
            continue;
        }
        println!("GRID {gid}", gid = gid.to_idx());
        if args.tiles {
            for ((r, c), tile) in grid.tiles.indexed_iter() {
                println!(
                    "\tTILE ({r}, {c}): {name} {kind} ({w}, {h}) at ({x}, {y})",
                    name = tile.name,
                    kind = tile.kind,
                    w = tile.width,
                    h = tile.height,
                    x = tile.x,
                    y = tile.y,
                );
                if args.sites {
                    for site in &tile.sites {
                        println!(
                            "\t\tSITE {name} ({x}, {y})",
                            name = site.name,
                            x = site.x,
                            y = site.y
                        );
                    }
                }
            }
        }
        if args.nodes {
            for (_, nn, node) in &grid.nodes {
                print!("\tNODE {nn}");
                if let Some(typ) = node.typ {
                    print!(" TYPE {typ}");
                }
                println!();
                for alias in &node.aliases {
                    println!("\t\tALIAS {alias}");
                }
                if let Some((ref site, ref pin, dir)) = node.pin {
                    println!("\t\tPIN {site} {pin} {dir:?}");
                }
            }
        }
        if args.pips {
            for (&(wf, wt), pip) in grid.pips.iter().sorted_by_key(|(&k, _)| k) {
                print!(
                    "\tPIP {wt} <- {wf}",
                    wt = grid.nodes.key(wt),
                    wf = grid.nodes.key(wf)
                );
                if pip.is_j {
                    print!(" IS_J");
                }
                if let Some(buf) = pip.buf {
                    print!(" BUF {buf}", buf = grid.bufs[buf]);
                }
                println!();
            }
        }
    }
    Ok(())
}
