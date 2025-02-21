use std::{
    collections::{BTreeMap, BTreeSet, btree_map},
    sync::LazyLock,
};

use itertools::Itertools;
use prjcombine_re_xilinx_geom::GeomDb;
use prjcombine_types::tiledb::TileDb;
use prjcombine_virtex::{
    bond::Bond,
    chip::{Chip, ChipKind, DisabledPart},
    db::{Database, DeviceCombo, Part},
};
use regex::Regex;
use unnamed_entity::{EntityMap, EntitySet, EntityVec};

struct TmpPart<'a> {
    grid: &'a Chip,
    bonds: BTreeMap<&'a str, &'a Bond>,
    speeds: BTreeSet<&'a str>,
    combos: BTreeSet<(&'a str, &'a str)>,
    disabled: BTreeSet<DisabledPart>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum PartKind {
    Virtex,
    QVirtex,
    QRVirtex,
    Spartan2,
    ASpartan2,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct SortKey<'a> {
    kind: ChipKind,
    width: usize,
    height: usize,
    part_kind: PartKind,
    name: &'a str,
}

static RE_VIRTEX: LazyLock<Regex> = LazyLock::new(|| Regex::new("^xcv[0-9]+e?$").unwrap());
static RE_QVIRTEX: LazyLock<Regex> = LazyLock::new(|| Regex::new("^xqv[0-9]+e?$").unwrap());
static RE_QRVIRTEX: LazyLock<Regex> = LazyLock::new(|| Regex::new("^xqvr[0-9]+e?$").unwrap());
static RE_SPARTAN2: LazyLock<Regex> = LazyLock::new(|| Regex::new("^xc2s[0-9]+e?$").unwrap());
static RE_ASPARTAN2: LazyLock<Regex> = LazyLock::new(|| Regex::new("^xa2s[0-9]+e?$").unwrap());

fn sort_key<'a>(name: &'a str, grid: &'a Chip) -> SortKey<'a> {
    let part_kind = if RE_VIRTEX.is_match(name) {
        PartKind::Virtex
    } else if RE_QVIRTEX.is_match(name) {
        PartKind::QVirtex
    } else if RE_QRVIRTEX.is_match(name) {
        PartKind::QRVirtex
    } else if RE_SPARTAN2.is_match(name) {
        PartKind::Spartan2
    } else if RE_ASPARTAN2.is_match(name) {
        PartKind::ASpartan2
    } else {
        panic!("ummm {name}?")
    };
    SortKey {
        kind: grid.kind,
        width: grid.columns,
        height: grid.rows,
        part_kind,
        name,
    }
}

pub fn finish(geom: GeomDb, tiledb: TileDb) -> Database {
    let mut tmp_parts: BTreeMap<&str, _> = BTreeMap::new();
    for dev in &geom.devices {
        let prjcombine_re_xilinx_geom::Grid::Virtex(ref grid) =
            geom.grids[*dev.grids.first().unwrap()]
        else {
            unreachable!()
        };
        let disabled: BTreeSet<_> = dev
            .disabled
            .iter()
            .map(|&dis| {
                let prjcombine_re_xilinx_geom::DisabledPart::Virtex(dis) = dis else {
                    unreachable!()
                };
                dis
            })
            .collect();
        let tpart = tmp_parts.entry(&dev.name).or_insert_with(|| TmpPart {
            grid,
            disabled: disabled.clone(),
            bonds: Default::default(),
            speeds: Default::default(),
            combos: Default::default(),
        });
        assert_eq!(tpart.grid, grid);
        assert_eq!(tpart.disabled, disabled);
        for devbond in dev.bonds.values() {
            let prjcombine_re_xilinx_geom::Bond::Virtex(ref bond) = geom.bonds[devbond.bond] else {
                unreachable!()
            };
            match tpart.bonds.entry(&devbond.name) {
                btree_map::Entry::Vacant(entry) => {
                    entry.insert(bond);
                }
                btree_map::Entry::Occupied(entry) => {
                    assert_eq!(*entry.get(), bond);
                }
            }
        }
        for speed in dev.speeds.values() {
            tpart.speeds.insert(speed);
        }
        for combo in &dev.combos {
            tpart.combos.insert((
                &dev.bonds[combo.devbond_idx].name,
                &dev.speeds[combo.speed_idx],
            ));
        }
    }
    let mut grids = EntitySet::new();
    let mut bonds = EntitySet::new();
    let mut parts = vec![];
    for (name, tpart) in tmp_parts
        .into_iter()
        .sorted_by_key(|(name, tpart)| sort_key(name, tpart.grid))
    {
        let grid = grids.insert(tpart.grid.clone()).0;
        let mut dev_bonds = EntityMap::new();
        for (bname, bond) in tpart.bonds {
            let bond = bonds.insert(bond.clone()).0;
            dev_bonds.insert(bname.to_string(), bond);
        }
        let mut speeds = EntitySet::new();
        for speed in tpart.speeds {
            speeds.insert(speed.to_string());
        }
        let mut combos = vec![];
        for combo in tpart.combos {
            combos.push(DeviceCombo {
                devbond: dev_bonds.get(combo.0).unwrap().0,
                speed: speeds.get(combo.1).unwrap(),
            });
        }
        let speeds = EntityVec::from_iter(speeds.into_values());
        let part = Part {
            name: name.into(),
            chip: grid,
            bonds: dev_bonds,
            speeds,
            combos,
            disabled: tpart.disabled,
        };
        parts.push(part);
    }
    let grids = grids.into_vec();
    let bonds = bonds.into_vec();

    assert_eq!(geom.ints.len(), 1);
    let int = geom.ints.into_values().next().unwrap();

    // TODO: resort int

    Database {
        chips: grids,
        bonds,
        parts,
        int,
        tiles: tiledb,
    }
}
