use prjcombine_re_collector::{xlat_bit, xlat_enum, Diff, OcdMode};
use prjcombine_re_hammer::Session;
use prjcombine_re_xilinx_geom::ExpandedDevice;

use crate::{
    backend::IseBackend,
    diff::CollectorCtx,
    fgen::{TileBits, TileRelation},
    fuzz::FuzzCtx,
    fuzz_enum, fuzz_inv, fuzz_multi, fuzz_one,
};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Mode {
    Virtex5,
    Virtex6,
    Virtex7,
    Spartan6,
}

pub fn add_fuzzers<'a>(session: &mut Session<IseBackend<'a>>, backend: &IseBackend<'a>) {
    let mode = match backend.edev {
        ExpandedDevice::Virtex4(edev) => match edev.kind {
            prjcombine_virtex4::grid::GridKind::Virtex4 => unreachable!(),
            prjcombine_virtex4::grid::GridKind::Virtex5 => Mode::Virtex5,
            prjcombine_virtex4::grid::GridKind::Virtex6 => Mode::Virtex6,
            prjcombine_virtex4::grid::GridKind::Virtex7 => Mode::Virtex7,
        },
        ExpandedDevice::Spartan6(_) => Mode::Spartan6,
        _ => unreachable!(),
    };

    for tile_name in if mode == Mode::Spartan6 {
        ["CLEXL", "CLEXM"]
    } else {
        ["CLBLL", "CLBLM"]
    } {
        let node_kind = backend.egrid.db.get_node(tile_name);
        if backend.egrid.node_index[node_kind].is_empty() {
            continue;
        }
        let bk_x = if mode == Mode::Spartan6 {
            "SLICEX"
        } else {
            "SLICEL"
        };
        for i in 0..2 {
            let ctx = FuzzCtx::new(
                session,
                backend,
                tile_name,
                format!("SLICE{i}"),
                TileBits::MainAuto,
            );
            let is_x = i == 1 && mode == Mode::Spartan6;
            let is_m = i == 0 && tile_name.ends_with('M');

            // LUTs
            for attr in ["A6LUT", "B6LUT", "C6LUT", "D6LUT"] {
                fuzz_multi!(ctx, attr, "#LUT", 64, [(mode bk_x)], (attr_lut attr));
            }

            if is_m {
                // LUT RAM
                fuzz_enum!(ctx, "WEMUX", ["WE", "CE"], [
                    (mode "SLICEM"),
                    (attr "A6LUT", "#RAM:0"),
                    (attr "A6RAMMODE", "SPRAM64"),
                    (pin "WE"),
                    (pin "CE")
                ]);
                for attr in ["WA7USED", "WA8USED"] {
                    fuzz_enum!(ctx, attr, ["0"], [
                        (mode "SLICEM"),
                        (attr "A6LUT", "#RAM:0"),
                        (attr "A6RAMMODE", "SPRAM64"),
                        (pin "AX"),
                        (pin "BX"),
                        (pin "CX"),
                        (pin "DX")
                    ]);
                }
                if matches!(mode, Mode::Virtex5 | Mode::Spartan6) {
                    fuzz_enum!(ctx, "ADI1MUX", ["AX", "BMC31", "BDI1"], [
                        (mode "SLICEM"),
                        (attr "A6LUT", "#RAM:0"),
                        (attr "A6RAMMODE", "SPRAM64"),
                        (pin "AX")
                    ]);
                    fuzz_enum!(ctx, "BDI1MUX", ["BX", "CMC31", "DX"], [
                        (mode "SLICEM"),
                        (attr "B6LUT", "#RAM:0"),
                        (attr "B6RAMMODE", "SPRAM64"),
                        (pin "BX"),
                        (pin "DX")
                    ]);
                    fuzz_enum!(ctx, "CDI1MUX", ["CX", "DMC31", "DX"], [
                        (mode "SLICEM"),
                        (attr "C6LUT", "#RAM:0"),
                        (attr "C6RAMMODE", "SPRAM64"),
                        (pin "CX"),
                        (pin "DX")
                    ]);
                } else {
                    fuzz_enum!(ctx, "ADI1MUX", ["AI", "BMC31", "BDI1"], [
                        (mode "SLICEM"),
                        (attr "A6LUT", "#RAM:0"),
                        (attr "A6RAMMODE", "SPRAM64"),
                        (pin "AI")
                    ]);
                    fuzz_enum!(ctx, "BDI1MUX", ["BI", "CMC31", "DI"], [
                        (mode "SLICEM"),
                        (attr "B6LUT", "#RAM:0"),
                        (attr "B6RAMMODE", "SPRAM64"),
                        (pin "BI"),
                        (pin "DI")
                    ]);
                    fuzz_enum!(ctx, "CDI1MUX", ["CI", "DMC31", "DI"], [
                        (mode "SLICEM"),
                        (attr "C6LUT", "#RAM:0"),
                        (attr "C6RAMMODE", "SPRAM64"),
                        (pin "CI"),
                        (pin "DI")
                    ]);
                }
                fuzz_enum!(ctx, "A6RAMMODE", ["SPRAM32", "SPRAM64", "DPRAM32", "DPRAM64", "SRL16", "SRL32"], [
                    (mode "SLICEM"),
                    (attr "A6LUT", "#RAM:0")
                ]);
                fuzz_enum!(ctx, "B6RAMMODE", ["SPRAM32", "SPRAM64", "DPRAM32", "DPRAM64", "SRL16", "SRL32"], [
                    (mode "SLICEM"),
                    (attr "B6LUT", "#RAM:0")
                ]);
                fuzz_enum!(ctx, "C6RAMMODE", ["SPRAM32", "SPRAM64", "DPRAM32", "DPRAM64", "SRL16", "SRL32"], [
                    (mode "SLICEM"),
                    (attr "C6LUT", "#RAM:0")
                ]);
                fuzz_enum!(ctx, "D6RAMMODE", ["SPRAM32", "SPRAM64", "DPRAM32", "DPRAM64", "SRL16", "SRL32"], [
                    (mode "SLICEM"),
                    (attr "D6LUT", "#RAM:0")
                ]);
            }

            if !is_x {
                // carry chain
                fuzz_enum!(ctx, "ACY0", ["AX", "O5"], [
                    (mode "SLICEL"),
                    (attr "A5LUT", "#LUT:0"),
                    (attr "A6LUT", "#LUT:0"),
                    (attr "COUTUSED", "0"),
                    (pin "AX"),
                    (pin "COUT")
                ]);
                fuzz_enum!(ctx, "BCY0", ["BX", "O5"], [
                    (mode "SLICEL"),
                    (attr "B5LUT", "#LUT:0"),
                    (attr "B6LUT", "#LUT:0"),
                    (attr "COUTUSED", "0"),
                    (pin "BX"),
                    (pin "COUT")
                ]);
                fuzz_enum!(ctx, "CCY0", ["CX", "O5"], [
                    (mode "SLICEL"),
                    (attr "C5LUT", "#LUT:0"),
                    (attr "C6LUT", "#LUT:0"),
                    (attr "COUTUSED", "0"),
                    (pin "CX"),
                    (pin "COUT")
                ]);
                fuzz_enum!(ctx, "DCY0", ["DX", "O5"], [
                    (mode "SLICEL"),
                    (attr "D5LUT", "#LUT:0"),
                    (attr "D6LUT", "#LUT:0"),
                    (attr "COUTUSED", "0"),
                    (pin "DX"),
                    (pin "COUT")
                ]);
                fuzz_enum!(ctx, "PRECYINIT", ["AX", "1", "0"], [
                    (mode "SLICEL"),
                    (attr "COUTUSED", "0"),
                    (pin "AX"),
                    (pin "COUT")
                ]);

                fuzz_one!(ctx, "CINUSED", "1", [], [
                    (related TileRelation::ClbCinDown, (pip (pin "COUT"), (pin_far "COUT")))
                ]);
            }

            // misc muxes
            if is_x {
                fuzz_enum!(ctx, "AOUTMUX", ["A5Q", "O5"], [
                    (mode "SLICEX"),
                    (attr "A6LUT", "#LUT:0"),
                    (attr "A5LUT", "#LUT:0"),
                    (pin "AMUX")
                ]);
                fuzz_enum!(ctx, "BOUTMUX", ["B5Q", "O5"], [
                    (mode "SLICEX"),
                    (attr "B6LUT", "#LUT:0"),
                    (attr "B5LUT", "#LUT:0"),
                    (pin "BMUX")
                ]);
                fuzz_enum!(ctx, "COUTMUX", ["C5Q", "O5"], [
                    (mode "SLICEX"),
                    (attr "C6LUT", "#LUT:0"),
                    (attr "C5LUT", "#LUT:0"),
                    (pin "CMUX")
                ]);
                fuzz_enum!(ctx, "DOUTMUX", ["D5Q", "O5"], [
                    (mode "SLICEX"),
                    (attr "D6LUT", "#LUT:0"),
                    (attr "D5LUT", "#LUT:0"),
                    (pin "DMUX")
                ]);
                fuzz_enum!(ctx, "AFFMUX", ["AX", "O6"], [
                    (mode "SLICEX"),
                    (attr "A6LUT", "#LUT:0"),
                    (attr "AFF", "#FF"),
                    (pin "AX"),
                    (pin "AQ"),
                    (pin "CLK")
                ]);
                fuzz_enum!(ctx, "BFFMUX", ["BX", "O6"], [
                    (mode "SLICEX"),
                    (attr "B6LUT", "#LUT:0"),
                    (attr "BFF", "#FF"),
                    (pin "BX"),
                    (pin "BQ"),
                    (pin "CLK")
                ]);
                fuzz_enum!(ctx, "CFFMUX", ["CX", "O6"], [
                    (mode "SLICEX"),
                    (attr "C6LUT", "#LUT:0"),
                    (attr "CFF", "#FF"),
                    (pin "CX"),
                    (pin "CQ"),
                    (pin "CLK")
                ]);
                fuzz_enum!(ctx, "DFFMUX", ["DX", "O6"], [
                    (mode "SLICEX"),
                    (attr "D6LUT", "#LUT:0"),
                    (attr "DFF", "#FF"),
                    (pin "DX"),
                    (pin "DQ"),
                    (pin "CLK")
                ]);
            } else {
                // [ABCD]MUX
                if mode == Mode::Virtex5 {
                    fuzz_enum!(ctx, "AOUTMUX", ["O5", "O6", "XOR", "CY", "F7"], [
                        (mode "SLICEL"),
                        (attr "A6LUT", "#LUT:0"),
                        (attr "A5LUT", "#LUT:0"),
                        (pin "AMUX")
                    ]);
                    fuzz_enum!(ctx, "BOUTMUX", ["O5", "O6", "XOR", "CY", "F8"], [
                        (mode "SLICEL"),
                        (attr "B6LUT", "#LUT:0"),
                        (attr "B5LUT", "#LUT:0"),
                        (pin "BMUX")
                    ]);
                    fuzz_enum!(ctx, "COUTMUX", ["O5", "O6", "XOR", "CY", "F7"], [
                        (mode "SLICEL"),
                        (attr "C6LUT", "#LUT:0"),
                        (attr "C5LUT", "#LUT:0"),
                        (pin "CMUX")
                    ]);
                    if is_m {
                        fuzz_enum!(ctx, "DOUTMUX", ["O5", "O6", "XOR", "CY", "MC31"], [
                            (mode "SLICEM"),
                            (attr "A6LUT", "#LUT:0"),
                            (attr "D6LUT", "#LUT:0"),
                            (attr "D5LUT", "#LUT:0"),
                            (pin "DMUX")
                        ]);
                    } else {
                        fuzz_enum!(ctx, "DOUTMUX", ["O5", "O6", "XOR", "CY"], [
                            (mode "SLICEL"),
                            (attr "D6LUT", "#LUT:0"),
                            (attr "D5LUT", "#LUT:0"),
                            (pin "DMUX")
                        ]);
                    }
                } else {
                    fuzz_enum!(ctx, "AOUTMUX", ["O5", "O6", "XOR", "CY", "A5Q", "F7"], [
                        (mode "SLICEL"),
                        (attr "A6LUT", "#LUT:0"),
                        (attr "A5LUT", "#LUT:0"),
                        (attr "A5FFMUX", ""),
                        (attr "CLKINV", "CLK"),
                        (pin "AMUX"),
                        (pin "CLK")
                    ]);
                    fuzz_enum!(ctx, "BOUTMUX", ["O5", "O6", "XOR", "CY", "B5Q", "F8"], [
                        (mode "SLICEL"),
                        (attr "B6LUT", "#LUT:0"),
                        (attr "B5LUT", "#LUT:0"),
                        (attr "B5FFMUX", ""),
                        (attr "CLKINV", "CLK"),
                        (pin "BMUX"),
                        (pin "CLK")
                    ]);
                    fuzz_enum!(ctx, "COUTMUX", ["O5", "O6", "XOR", "CY", "C5Q", "F7"], [
                        (mode "SLICEL"),
                        (attr "C6LUT", "#LUT:0"),
                        (attr "C5LUT", "#LUT:0"),
                        (attr "C5FFMUX", ""),
                        (attr "CLKINV", "CLK"),
                        (pin "CMUX"),
                        (pin "CLK")
                    ]);
                    if is_m {
                        fuzz_enum!(ctx, "DOUTMUX", ["O5", "O6", "XOR", "CY", "D5Q", "MC31"], [
                            (mode "SLICEM"),
                            (attr "A6LUT", "#LUT:0"),
                            (attr "D6LUT", "#LUT:0"),
                            (attr "D5LUT", "#LUT:0"),
                            (attr "D5FFMUX", ""),
                            (attr "CLKINV", "CLK"),
                            (pin "DMUX"),
                            (pin "CLK")
                        ]);
                    } else {
                        fuzz_enum!(ctx, "DOUTMUX", ["O5", "O6", "XOR", "CY", "D5Q"], [
                            (mode "SLICEL"),
                            (attr "D6LUT", "#LUT:0"),
                            (attr "D5LUT", "#LUT:0"),
                            (attr "D5FFMUX", ""),
                            (attr "CLKINV", "CLK"),
                            (pin "DMUX"),
                            (pin "CLK")
                        ]);
                    }
                }

                // [ABCD]FF input
                fuzz_enum!(ctx, "AFFMUX", ["O5", "O6", "XOR", "CY", "AX", "F7"], [
                    (mode "SLICEL"),
                    (attr "A6LUT", "#LUT:0"),
                    (attr "A5LUT", "#LUT:0"),
                    (attr "AFF", "#FF"),
                    (attr "CLKINV", "CLK"),
                    (pin "AX"),
                    (pin "AQ"),
                    (pin "CLK")
                ]);
                fuzz_enum!(ctx, "BFFMUX", ["O5", "O6", "XOR", "CY", "BX", "F8"], [
                    (mode "SLICEL"),
                    (attr "B6LUT", "#LUT:0"),
                    (attr "B5LUT", "#LUT:0"),
                    (attr "BFF", "#FF"),
                    (attr "CLKINV", "CLK"),
                    (pin "BX"),
                    (pin "BQ"),
                    (pin "CLK")
                ]);
                fuzz_enum!(ctx, "CFFMUX", ["O5", "O6", "XOR", "CY", "CX", "F7"], [
                    (mode "SLICEL"),
                    (attr "C6LUT", "#LUT:0"),
                    (attr "C5LUT", "#LUT:0"),
                    (attr "CFF", "#FF"),
                    (attr "CLKINV", "CLK"),
                    (pin "CX"),
                    (pin "CQ"),
                    (pin "CLK")
                ]);
                if is_m {
                    fuzz_enum!(ctx, "DFFMUX", ["O5", "O6", "XOR", "CY", "DX", "MC31"], [
                        (mode "SLICEM"),
                        (attr "A6LUT", "#LUT:0"),
                        (attr "D6LUT", "#LUT:0"),
                        (attr "D5LUT", "#LUT:0"),
                        (attr "DFF", "#FF"),
                        (attr "CLKINV", "CLK"),
                        (pin "DX"),
                        (pin "DQ"),
                        (pin "CLK")
                    ]);
                } else {
                    fuzz_enum!(ctx, "DFFMUX", ["O5", "O6", "XOR", "CY", "DX"], [
                        (mode "SLICEL"),
                        (attr "D6LUT", "#LUT:0"),
                        (attr "D5LUT", "#LUT:0"),
                        (attr "DFF", "#FF"),
                        (attr "CLKINV", "CLK"),
                        (pin "DX"),
                        (pin "DQ"),
                        (pin "CLK")
                    ]);
                }
                if matches!(mode, Mode::Virtex6 | Mode::Virtex7) {
                    fuzz_enum!(ctx, "A5FFMUX", ["IN_A", "IN_B"], [
                        (mode "SLICEL"),
                        (attr "A6LUT", "#LUT:0"),
                        (attr "A5LUT", "#LUT:0"),
                        (attr "AOUTMUX", "A5Q"),
                        (attr "CLKINV", "CLK"),
                        (pin "AX"),
                        (pin "AMUX"),
                        (pin "CLK")
                    ]);
                    fuzz_enum!(ctx, "B5FFMUX", ["IN_A", "IN_B"], [
                        (mode "SLICEL"),
                        (attr "B6LUT", "#LUT:0"),
                        (attr "B5LUT", "#LUT:0"),
                        (attr "BOUTMUX", "B5Q"),
                        (attr "CLKINV", "CLK"),
                        (pin "BX"),
                        (pin "BMUX"),
                        (pin "CLK")
                    ]);
                    fuzz_enum!(ctx, "C5FFMUX", ["IN_A", "IN_B"], [
                        (mode "SLICEL"),
                        (attr "C6LUT", "#LUT:0"),
                        (attr "C5LUT", "#LUT:0"),
                        (attr "COUTMUX", "C5Q"),
                        (attr "CLKINV", "CLK"),
                        (pin "CX"),
                        (pin "CMUX"),
                        (pin "CLK")
                    ]);
                    fuzz_enum!(ctx, "D5FFMUX", ["IN_A", "IN_B"], [
                        (mode "SLICEL"),
                        (attr "D6LUT", "#LUT:0"),
                        (attr "D5LUT", "#LUT:0"),
                        (attr "DOUTMUX", "D5Q"),
                        (attr "CLKINV", "CLK"),
                        (pin "DX"),
                        (pin "DMUX"),
                        (pin "CLK")
                    ]);
                }
            }

            // FFs
            fuzz_enum!(ctx, "SYNC_ATTR", ["SYNC", "ASYNC"], [
                (mode bk_x),
                (attr "AFF", "#FF"),
                (pin "AQ")
            ]);
            fuzz_inv!(ctx, "CLK", [
                (mode bk_x),
                (attr "AFF", "#FF"),
                (pin "AQ")
            ]);
            match mode {
                Mode::Virtex5 => {
                    fuzz_enum!(ctx, "REVUSED", ["0"], [
                        (mode bk_x),
                        (attr "AFF", "#FF"),
                        (pin "AQ"),
                        (pin "DX"),
                        (pin "CLK")
                    ]);
                    fuzz_enum!(ctx, "AFF", ["#LATCH", "#FF"], [
                        (mode bk_x),
                        (attr "AFFINIT", "INIT1"),
                        (attr "BFF", ""),
                        (attr "CFF", ""),
                        (attr "DFF", ""),
                        (pin "AQ"),
                        (pin "CLK")
                    ]);
                    fuzz_enum!(ctx, "BFF", ["#LATCH", "#FF"], [
                        (mode bk_x),
                        (attr "BFFINIT", "INIT1"),
                        (attr "AFF", ""),
                        (attr "CFF", ""),
                        (attr "DFF", ""),
                        (pin "BQ"),
                        (pin "CLK")
                    ]);
                    fuzz_enum!(ctx, "CFF", ["#LATCH", "#FF"], [
                        (mode bk_x),
                        (attr "CFFINIT", "INIT1"),
                        (attr "AFF", ""),
                        (attr "BFF", ""),
                        (attr "DFF", ""),
                        (pin "CQ"),
                        (pin "CLK")
                    ]);
                    fuzz_enum!(ctx, "DFF", ["#LATCH", "#FF"], [
                        (mode bk_x),
                        (attr "DFFINIT", "INIT1"),
                        (attr "AFF", ""),
                        (attr "BFF", ""),
                        (attr "CFF", ""),
                        (pin "DQ"),
                        (pin "CLK")
                    ]);
                    for attr in ["AFFSR", "BFFSR", "CFFSR", "DFFSR"] {
                        fuzz_enum!(ctx, attr, ["SRHIGH", "SRLOW"], [
                            (mode bk_x),
                            (attr "AFF", "#FF"),
                            (attr "BFF", "#FF"),
                            (attr "CFF", "#FF"),
                            (attr "DFF", "#FF"),
                            (attr "AFFINIT", "INIT0"),
                            (attr "BFFINIT", "INIT0"),
                            (attr "CFFINIT", "INIT0"),
                            (attr "DFFINIT", "INIT0"),
                            (pin "AQ"),
                            (pin "BQ"),
                            (pin "CQ"),
                            (pin "DQ"),
                            (pin "CLK")
                        ]);
                    }
                    for attr in ["AFFINIT", "BFFINIT", "CFFINIT", "DFFINIT"] {
                        fuzz_enum!(ctx, attr, ["INIT0", "INIT1"], [
                            (mode bk_x),
                            (attr "AFF", "#FF"),
                            (attr "BFF", "#FF"),
                            (attr "CFF", "#FF"),
                            (attr "DFF", "#FF"),
                            (attr "AFFSR", "SRLOW"),
                            (attr "BFFSR", "SRLOW"),
                            (attr "CFFSR", "SRLOW"),
                            (attr "DFFSR", "SRLOW"),
                            (pin "AQ"),
                            (pin "BQ"),
                            (pin "CQ"),
                            (pin "DQ"),
                            (pin "CLK")
                        ]);
                    }
                }
                Mode::Spartan6 => {
                    fuzz_enum!(ctx, "AFF", ["#LATCH", "#FF", "AND2L", "OR2L"], [
                        (mode bk_x),
                        (attr "BFF", ""),
                        (attr "CFF", ""),
                        (attr "DFF", ""),
                        (pin "AQ"),
                        (pin "CLK")
                    ]);
                    fuzz_enum!(ctx, "BFF", ["#LATCH", "#FF", "AND2L", "OR2L"], [
                        (mode bk_x),
                        (attr "AFF", ""),
                        (attr "CFF", ""),
                        (attr "DFF", ""),
                        (pin "BQ"),
                        (pin "CLK")
                    ]);
                    fuzz_enum!(ctx, "CFF", ["#LATCH", "#FF", "AND2L", "OR2L"], [
                        (mode bk_x),
                        (attr "AFF", ""),
                        (attr "BFF", ""),
                        (attr "DFF", ""),
                        (pin "CQ"),
                        (pin "CLK")
                    ]);
                    fuzz_enum!(ctx, "DFF", ["#LATCH", "#FF", "AND2L", "OR2L"], [
                        (mode bk_x),
                        (attr "AFF", ""),
                        (attr "BFF", ""),
                        (attr "CFF", ""),
                        (pin "DQ"),
                        (pin "CLK")
                    ]);
                    for attr in ["AFFSRINIT", "BFFSRINIT", "CFFSRINIT", "DFFSRINIT"] {
                        fuzz_enum!(ctx, attr, ["SRINIT0", "SRINIT1"], [
                            (mode bk_x),
                            (attr "AFF", "#FF"),
                            (attr "BFF", "#FF"),
                            (attr "CFF", "#FF"),
                            (attr "DFF", "#FF"),
                            (pin "AQ"),
                            (pin "BQ"),
                            (pin "CQ"),
                            (pin "DQ"),
                            (pin "CLK")
                        ]);
                    }
                    fuzz_enum!(ctx, "A5FFSRINIT", ["SRINIT0", "SRINIT1"], [
                        (mode bk_x),
                        (attr "AOUTMUX", "A5Q"),
                        (attr "A5LUT", "#LUT:0"),
                        (attr "A6LUT", "#LUT:0"),
                        (pin "AMUX"),
                        (pin "CLK")
                    ]);
                    fuzz_enum!(ctx, "B5FFSRINIT", ["SRINIT0", "SRINIT1"], [
                        (mode bk_x),
                        (attr "BOUTMUX", "B5Q"),
                        (attr "B5LUT", "#LUT:0"),
                        (attr "B6LUT", "#LUT:0"),
                        (pin "BMUX"),
                        (pin "CLK")
                    ]);
                    fuzz_enum!(ctx, "C5FFSRINIT", ["SRINIT0", "SRINIT1"], [
                        (mode bk_x),
                        (attr "COUTMUX", "C5Q"),
                        (attr "C5LUT", "#LUT:0"),
                        (attr "C6LUT", "#LUT:0"),
                        (pin "CMUX"),
                        (pin "CLK")
                    ]);
                    fuzz_enum!(ctx, "D5FFSRINIT", ["SRINIT0", "SRINIT1"], [
                        (mode bk_x),
                        (attr "DOUTMUX", "D5Q"),
                        (attr "D5LUT", "#LUT:0"),
                        (attr "D6LUT", "#LUT:0"),
                        (pin "DMUX"),
                        (pin "CLK")
                    ]);
                }
                Mode::Virtex6 | Mode::Virtex7 => {
                    fuzz_enum!(ctx, "AFF", ["#LATCH", "#FF", "AND2L", "OR2L"], [
                        (mode bk_x),
                        (attr "AFFINIT", "INIT1"),
                        (attr "BFF", ""),
                        (attr "CFF", ""),
                        (attr "DFF", ""),
                        (pin "AQ"),
                        (pin "CLK")
                    ]);
                    fuzz_enum!(ctx, "BFF", ["#LATCH", "#FF", "AND2L", "OR2L"], [
                        (mode bk_x),
                        (attr "BFFINIT", "INIT1"),
                        (attr "AFF", ""),
                        (attr "CFF", ""),
                        (attr "DFF", ""),
                        (pin "BQ"),
                        (pin "CLK")
                    ]);
                    fuzz_enum!(ctx, "CFF", ["#LATCH", "#FF", "AND2L", "OR2L"], [
                        (mode bk_x),
                        (attr "CFFINIT", "INIT1"),
                        (attr "AFF", ""),
                        (attr "BFF", ""),
                        (attr "DFF", ""),
                        (pin "CQ"),
                        (pin "CLK")
                    ]);
                    fuzz_enum!(ctx, "DFF", ["#LATCH", "#FF", "AND2L", "OR2L"], [
                        (mode bk_x),
                        (attr "DFFINIT", "INIT1"),
                        (attr "AFF", ""),
                        (attr "BFF", ""),
                        (attr "CFF", ""),
                        (pin "DQ"),
                        (pin "CLK")
                    ]);

                    for attr in ["AFFSR", "BFFSR", "CFFSR", "DFFSR"] {
                        fuzz_enum!(ctx, attr, ["SRHIGH", "SRLOW"], [
                            (mode bk_x),
                            (attr "AFF", "#FF"),
                            (attr "BFF", "#FF"),
                            (attr "CFF", "#FF"),
                            (attr "DFF", "#FF"),
                            (attr "AFFINIT", "INIT0"),
                            (attr "BFFINIT", "INIT0"),
                            (attr "CFFINIT", "INIT0"),
                            (attr "DFFINIT", "INIT0"),
                            (pin "AQ"),
                            (pin "BQ"),
                            (pin "CQ"),
                            (pin "DQ"),
                            (pin "CLK")
                        ]);
                    }
                    for attr in ["AFFINIT", "BFFINIT", "CFFINIT", "DFFINIT"] {
                        fuzz_enum!(ctx, attr, ["INIT0", "INIT1"], [
                            (mode bk_x),
                            (attr "AFF", "#FF"),
                            (attr "BFF", "#FF"),
                            (attr "CFF", "#FF"),
                            (attr "DFF", "#FF"),
                            (attr "AFFSR", "SRLOW"),
                            (attr "BFFSR", "SRLOW"),
                            (attr "CFFSR", "SRLOW"),
                            (attr "DFFSR", "SRLOW"),
                            (pin "AQ"),
                            (pin "BQ"),
                            (pin "CQ"),
                            (pin "DQ"),
                            (pin "CLK")
                        ]);
                    }
                    fuzz_enum!(ctx, "A5FFSR", ["SRLOW", "SRHIGH"], [
                        (mode bk_x),
                        (attr "AOUTMUX", "A5Q"),
                        (attr "A5LUT", "#LUT:0"),
                        (attr "A6LUT", "#LUT:0"),
                        (attr "A5FFMUX", "IN_A"),
                        (attr "A5FFINIT", "INIT0"),
                        (pin "AMUX"),
                        (pin "CLK")
                    ]);
                    fuzz_enum!(ctx, "B5FFSR", ["SRLOW", "SRHIGH"], [
                        (mode bk_x),
                        (attr "BOUTMUX", "B5Q"),
                        (attr "B5LUT", "#LUT:0"),
                        (attr "B6LUT", "#LUT:0"),
                        (attr "B5FFMUX", "IN_A"),
                        (attr "B5FFINIT", "INIT0"),
                        (pin "BMUX"),
                        (pin "CLK")
                    ]);
                    fuzz_enum!(ctx, "C5FFSR", ["SRLOW", "SRHIGH"], [
                        (mode bk_x),
                        (attr "COUTMUX", "C5Q"),
                        (attr "C5LUT", "#LUT:0"),
                        (attr "C6LUT", "#LUT:0"),
                        (attr "C5FFMUX", "IN_A"),
                        (attr "C5FFINIT", "INIT0"),
                        (pin "CMUX"),
                        (pin "CLK")
                    ]);
                    fuzz_enum!(ctx, "D5FFSR", ["SRLOW", "SRHIGH"], [
                        (mode bk_x),
                        (attr "DOUTMUX", "D5Q"),
                        (attr "D5LUT", "#LUT:0"),
                        (attr "D6LUT", "#LUT:0"),
                        (attr "D5FFMUX", "IN_A"),
                        (attr "D5FFINIT", "INIT0"),
                        (pin "DMUX"),
                        (pin "CLK")
                    ]);
                    fuzz_enum!(ctx, "A5FFINIT", ["INIT0", "INIT1"], [
                        (mode bk_x),
                        (attr "AOUTMUX", "A5Q"),
                        (attr "A5LUT", "#LUT:0"),
                        (attr "A6LUT", "#LUT:0"),
                        (attr "A5FFMUX", "IN_A"),
                        (attr "A5FFSR", "SRLOW"),
                        (pin "AMUX"),
                        (pin "CLK")
                    ]);
                    fuzz_enum!(ctx, "B5FFINIT", ["INIT0", "INIT1"], [
                        (mode bk_x),
                        (attr "BOUTMUX", "B5Q"),
                        (attr "B5LUT", "#LUT:0"),
                        (attr "B6LUT", "#LUT:0"),
                        (attr "B5FFMUX", "IN_A"),
                        (attr "B5FFSR", "SRLOW"),
                        (pin "BMUX"),
                        (pin "CLK")
                    ]);
                    fuzz_enum!(ctx, "C5FFINIT", ["INIT0", "INIT1"], [
                        (mode bk_x),
                        (attr "COUTMUX", "C5Q"),
                        (attr "C5LUT", "#LUT:0"),
                        (attr "C6LUT", "#LUT:0"),
                        (attr "C5FFMUX", "IN_A"),
                        (attr "C5FFSR", "SRLOW"),
                        (pin "CMUX"),
                        (pin "CLK")
                    ]);
                    fuzz_enum!(ctx, "D5FFINIT", ["INIT0", "INIT1"], [
                        (mode bk_x),
                        (attr "DOUTMUX", "D5Q"),
                        (attr "D5LUT", "#LUT:0"),
                        (attr "D6LUT", "#LUT:0"),
                        (attr "D5FFMUX", "IN_A"),
                        (attr "D5FFSR", "SRLOW"),
                        (pin "DMUX"),
                        (pin "CLK")
                    ]);
                }
            }
            if matches!(mode, Mode::Virtex5 | Mode::Spartan6) {
                fuzz_enum!(ctx, "CEUSED", ["0"], [
                    (mode bk_x),
                    (attr "AFF", "#FF"),
                    (pin "AQ"),
                    (pin "CE"),
                    (pin "CLK")
                ]);
                fuzz_enum!(ctx, "SRUSED", ["0"], [
                    (mode bk_x),
                    (attr "AFF", "#FF"),
                    (pin "AQ"),
                    (pin "SR"),
                    (pin "CLK")
                ]);
            } else {
                fuzz_enum!(ctx, "CEUSEDMUX", ["1", "IN"], [
                    (mode bk_x),
                    (attr "AFF", "#FF"),
                    (pin "AQ"),
                    (pin "CE"),
                    (pin "CLK")
                ]);
                fuzz_enum!(ctx, "SRUSEDMUX", ["0", "IN"], [
                    (mode bk_x),
                    (attr "AFF", "#FF"),
                    (pin "AQ"),
                    (pin "SR"),
                    (pin "CLK")
                ]);
            }
        }
    }
}

pub fn collect_fuzzers(ctx: &mut CollectorCtx) {
    let mode = match ctx.edev {
        ExpandedDevice::Virtex4(edev) => match edev.kind {
            prjcombine_virtex4::grid::GridKind::Virtex4 => unreachable!(),
            prjcombine_virtex4::grid::GridKind::Virtex5 => Mode::Virtex5,
            prjcombine_virtex4::grid::GridKind::Virtex6 => Mode::Virtex6,
            prjcombine_virtex4::grid::GridKind::Virtex7 => Mode::Virtex7,
        },
        ExpandedDevice::Spartan6(_) => Mode::Spartan6,
        _ => unreachable!(),
    };

    for tile in if mode == Mode::Spartan6 {
        ["CLEXL", "CLEXM"]
    } else {
        ["CLBLL", "CLBLM"]
    } {
        let node_kind = ctx.edev.egrid().db.get_node(tile);
        if ctx.edev.egrid().node_index[node_kind].is_empty() {
            continue;
        }
        for (idx, bel) in ["SLICE0", "SLICE1"].into_iter().enumerate() {
            let is_x = idx == 1 && mode == Mode::Spartan6;
            let is_m = idx == 0 && tile.ends_with('M');

            // LUTs
            ctx.collect_bitvec(tile, bel, "A6LUT", "#LUT");
            ctx.collect_bitvec(tile, bel, "B6LUT", "#LUT");
            ctx.collect_bitvec(tile, bel, "C6LUT", "#LUT");
            ctx.collect_bitvec(tile, bel, "D6LUT", "#LUT");

            // LUT RAM
            if is_m {
                ctx.collect_enum(tile, bel, "WEMUX", &["WE", "CE"]);
                for attr in ["WA7USED", "WA8USED"] {
                    let diff = ctx.state.get_diff(tile, bel, attr, "0");
                    ctx.tiledb.insert(tile, bel, attr, xlat_bit(diff));
                }
                let di_muxes = match mode {
                    Mode::Virtex5 | Mode::Spartan6 => [
                        ("ADI1MUX", "AX", "BMC31", "BDI1"),
                        ("BDI1MUX", "BX", "CMC31", "DX"),
                        ("CDI1MUX", "CX", "DMC31", "DX"),
                    ],
                    Mode::Virtex6 | Mode::Virtex7 => [
                        ("ADI1MUX", "AI", "BMC31", "BDI1"),
                        ("BDI1MUX", "BI", "CMC31", "DI"),
                        ("CDI1MUX", "CI", "DMC31", "DI"),
                    ],
                };
                for (attr, byp, alt_shift, alt_ram) in di_muxes {
                    let d_byp = ctx.state.get_diff(tile, bel, attr, byp);
                    let d_alt = ctx.state.get_diff(tile, bel, attr, alt_shift);
                    assert_eq!(d_alt, ctx.state.get_diff(tile, bel, attr, alt_ram));
                    ctx.tiledb.insert(
                        tile,
                        bel,
                        attr,
                        xlat_enum(vec![(byp, d_byp), ("ALT", d_alt)]),
                    );
                }
                for (dattr, sattr) in [
                    ("ARAMMODE", "A6RAMMODE"),
                    ("BRAMMODE", "B6RAMMODE"),
                    ("CRAMMODE", "C6RAMMODE"),
                    ("DRAMMODE", "D6RAMMODE"),
                ] {
                    let d_ram32 = ctx.state.get_diff(tile, bel, sattr, "SPRAM32");
                    let d_ram64 = ctx.state.get_diff(tile, bel, sattr, "SPRAM64");
                    let d_srl16 = ctx.state.get_diff(tile, bel, sattr, "SRL16");
                    let d_srl32 = ctx.state.get_diff(tile, bel, sattr, "SRL32");
                    assert_eq!(d_ram32, ctx.state.get_diff(tile, bel, sattr, "DPRAM32"));
                    assert_eq!(d_ram64, ctx.state.get_diff(tile, bel, sattr, "DPRAM64"));
                    ctx.tiledb.insert(
                        tile,
                        bel,
                        dattr,
                        xlat_enum(vec![
                            ("RAM32", d_ram32),
                            ("RAM64", d_ram64),
                            ("SRL16", d_srl16),
                            ("SRL32", d_srl32),
                        ]),
                    );
                }
            }

            // carry chain
            if !is_x {
                ctx.collect_enum(tile, bel, "ACY0", &["O5", "AX"]);
                ctx.collect_enum(tile, bel, "BCY0", &["O5", "BX"]);
                ctx.collect_enum(tile, bel, "CCY0", &["O5", "CX"]);
                ctx.collect_enum(tile, bel, "DCY0", &["O5", "DX"]);
                ctx.collect_enum(tile, bel, "PRECYINIT", &["AX", "1", "0"]);
                let item = xlat_enum(vec![
                    ("CIN", ctx.state.get_diff(tile, bel, "CINUSED", "1")),
                    ("PRECYINIT", Diff::default()),
                ]);
                ctx.tiledb.insert(tile, bel, "CYINIT", item);
            }

            // misc muxes
            if is_x {
                ctx.collect_enum(tile, bel, "AOUTMUX", &["O5", "A5Q"]);
                ctx.collect_enum(tile, bel, "BOUTMUX", &["O5", "B5Q"]);
                ctx.collect_enum(tile, bel, "COUTMUX", &["O5", "C5Q"]);
                ctx.collect_enum(tile, bel, "DOUTMUX", &["O5", "D5Q"]);
                ctx.collect_enum(tile, bel, "AFFMUX", &["O6", "AX"]);
                ctx.collect_enum(tile, bel, "BFFMUX", &["O6", "BX"]);
                ctx.collect_enum(tile, bel, "CFFMUX", &["O6", "CX"]);
                ctx.collect_enum(tile, bel, "DFFMUX", &["O6", "DX"]);
            } else {
                if mode == Mode::Virtex5 {
                    ctx.collect_enum_default_ocd(
                        tile,
                        bel,
                        "AOUTMUX",
                        &["O6", "O5", "XOR", "CY", "F7"],
                        "NONE",
                        OcdMode::Mux,
                    );
                    ctx.collect_enum_default_ocd(
                        tile,
                        bel,
                        "BOUTMUX",
                        &["O6", "O5", "XOR", "CY", "F8"],
                        "NONE",
                        OcdMode::Mux,
                    );
                    ctx.collect_enum_default_ocd(
                        tile,
                        bel,
                        "COUTMUX",
                        &["O6", "O5", "XOR", "CY", "F7"],
                        "NONE",
                        OcdMode::Mux,
                    );
                    if is_m {
                        ctx.collect_enum_default_ocd(
                            tile,
                            bel,
                            "DOUTMUX",
                            &["O6", "O5", "XOR", "CY", "MC31"],
                            "NONE",
                            OcdMode::Mux,
                        );
                    } else {
                        ctx.collect_enum_default_ocd(
                            tile,
                            bel,
                            "DOUTMUX",
                            &["O6", "O5", "XOR", "CY"],
                            "NONE",
                            OcdMode::Mux,
                        );
                    }
                } else {
                    ctx.collect_enum_default_ocd(
                        tile,
                        bel,
                        "AOUTMUX",
                        &["O6", "O5", "XOR", "CY", "A5Q", "F7"],
                        "NONE",
                        OcdMode::Mux,
                    );
                    ctx.collect_enum_default_ocd(
                        tile,
                        bel,
                        "BOUTMUX",
                        &["O6", "O5", "XOR", "CY", "B5Q", "F8"],
                        "NONE",
                        OcdMode::Mux,
                    );
                    ctx.collect_enum_default_ocd(
                        tile,
                        bel,
                        "COUTMUX",
                        &["O6", "O5", "XOR", "CY", "C5Q", "F7"],
                        "NONE",
                        OcdMode::Mux,
                    );
                    if is_m {
                        ctx.collect_enum_default_ocd(
                            tile,
                            bel,
                            "DOUTMUX",
                            &["O6", "O5", "XOR", "CY", "D5Q", "MC31"],
                            "NONE",
                            OcdMode::Mux,
                        );
                    } else {
                        ctx.collect_enum_default_ocd(
                            tile,
                            bel,
                            "DOUTMUX",
                            &["O6", "O5", "XOR", "CY", "D5Q"],
                            "NONE",
                            OcdMode::Mux,
                        );
                    }
                }
                if mode == Mode::Spartan6 {
                    ctx.collect_enum(tile, bel, "AFFMUX", &["O6", "O5", "XOR", "CY", "AX", "F7"]);
                    ctx.collect_enum(tile, bel, "BFFMUX", &["O6", "O5", "XOR", "CY", "BX", "F8"]);
                    ctx.collect_enum(tile, bel, "CFFMUX", &["O6", "O5", "XOR", "CY", "CX", "F7"]);
                    if is_m {
                        ctx.collect_enum(
                            tile,
                            bel,
                            "DFFMUX",
                            &["O6", "O5", "XOR", "CY", "DX", "MC31"],
                        );
                    } else {
                        ctx.collect_enum(tile, bel, "DFFMUX", &["O6", "O5", "XOR", "CY", "DX"]);
                    }
                } else {
                    ctx.collect_enum_default_ocd(
                        tile,
                        bel,
                        "AFFMUX",
                        &["O6", "O5", "XOR", "CY", "AX", "F7"],
                        "NONE",
                        OcdMode::Mux,
                    );
                    ctx.collect_enum_default_ocd(
                        tile,
                        bel,
                        "BFFMUX",
                        &["O6", "O5", "XOR", "CY", "BX", "F8"],
                        "NONE",
                        OcdMode::Mux,
                    );
                    ctx.collect_enum_default_ocd(
                        tile,
                        bel,
                        "CFFMUX",
                        &["O6", "O5", "XOR", "CY", "CX", "F7"],
                        "NONE",
                        OcdMode::Mux,
                    );
                    if is_m {
                        ctx.collect_enum_default_ocd(
                            tile,
                            bel,
                            "DFFMUX",
                            &["O6", "O5", "XOR", "CY", "DX", "MC31"],
                            "NONE",
                            OcdMode::Mux,
                        );
                    } else {
                        ctx.collect_enum_default_ocd(
                            tile,
                            bel,
                            "DFFMUX",
                            &["O6", "O5", "XOR", "CY", "DX"],
                            "NONE",
                            OcdMode::Mux,
                        );
                    }
                }
                if matches!(mode, Mode::Virtex6 | Mode::Virtex7) {
                    for (attr, byp) in [
                        ("A5FFMUX", "AX"),
                        ("B5FFMUX", "BX"),
                        ("C5FFMUX", "CX"),
                        ("D5FFMUX", "DX"),
                    ] {
                        let d_o5 = ctx.state.get_diff(tile, bel, attr, "IN_A");
                        let d_byp = ctx.state.get_diff(tile, bel, attr, "IN_B");
                        ctx.tiledb.insert(
                            tile,
                            bel,
                            attr,
                            xlat_enum(vec![("O5", d_o5), (byp, d_byp), ("NONE", Diff::default())]),
                        );
                    }
                }
            }

            // FFs
            let ff_sync = ctx.state.get_diff(tile, bel, "SYNC_ATTR", "SYNC");
            ctx.state
                .get_diff(tile, bel, "SYNC_ATTR", "ASYNC")
                .assert_empty();
            ctx.tiledb
                .insert(tile, bel, "FF_SR_SYNC", xlat_bit(ff_sync));
            ctx.collect_inv(tile, bel, "CLK");
            if mode == Mode::Virtex5 {
                let revused = ctx.state.get_diff(tile, bel, "REVUSED", "0");
                ctx.tiledb
                    .insert(tile, bel, "FF_REV_ENABLE", xlat_bit(revused));
            }
            if matches!(mode, Mode::Virtex5 | Mode::Spartan6) {
                let ceused = ctx.state.get_diff(tile, bel, "CEUSED", "0");
                ctx.tiledb
                    .insert(tile, bel, "FF_CE_ENABLE", xlat_bit(ceused));
                let srused = ctx.state.get_diff(tile, bel, "SRUSED", "0");
                ctx.tiledb
                    .insert(tile, bel, "FF_SR_ENABLE", xlat_bit(srused));
            } else {
                ctx.state
                    .get_diff(tile, bel, "CEUSEDMUX", "1")
                    .assert_empty();
                ctx.state
                    .get_diff(tile, bel, "SRUSEDMUX", "0")
                    .assert_empty();
                let ceused = ctx.state.get_diff(tile, bel, "CEUSEDMUX", "IN");
                ctx.tiledb
                    .insert(tile, bel, "FF_CE_ENABLE", xlat_bit(ceused));
                let srused = ctx.state.get_diff(tile, bel, "SRUSEDMUX", "IN");
                ctx.tiledb
                    .insert(tile, bel, "FF_SR_ENABLE", xlat_bit(srused));
            }
            if mode != Mode::Virtex6 {
                let ff_latch = ctx.state.get_diff(tile, bel, "AFF", "#LATCH");
                for attr in ["AFF", "BFF", "CFF", "DFF"] {
                    ctx.state.get_diff(tile, bel, attr, "#FF").assert_empty();
                    if attr != "AFF" {
                        assert_eq!(ff_latch, ctx.state.get_diff(tile, bel, attr, "#LATCH"));
                    }
                    if mode != Mode::Virtex5 {
                        assert_eq!(ff_latch, ctx.state.get_diff(tile, bel, attr, "AND2L"));
                        assert_eq!(ff_latch, ctx.state.get_diff(tile, bel, attr, "OR2L"));
                    }
                }
                ctx.tiledb.insert(tile, bel, "FF_LATCH", xlat_bit(ff_latch));
            } else {
                for attr in ["AFF", "BFF", "CFF", "DFF"] {
                    ctx.state.get_diff(tile, bel, attr, "#FF").assert_empty();
                    let ff_latch = ctx.state.get_diff(tile, bel, attr, "#LATCH");
                    assert_eq!(ff_latch, ctx.state.get_diff(tile, bel, attr, "AND2L"));
                    assert_eq!(ff_latch, ctx.state.get_diff(tile, bel, attr, "OR2L"));
                    ctx.tiledb
                        .insert(tile, bel, format!("{attr}_LATCH"), xlat_bit(ff_latch));
                }
            }
            match mode {
                Mode::Virtex5 => {
                    for attr in ["AFFINIT", "BFFINIT", "CFFINIT", "DFFINIT"] {
                        ctx.collect_enum_bool(tile, bel, attr, "INIT0", "INIT1");
                    }
                    for attr in ["AFFSR", "BFFSR", "CFFSR", "DFFSR"] {
                        ctx.collect_enum_bool(tile, bel, attr, "SRLOW", "SRHIGH");
                    }
                }
                Mode::Virtex6 | Mode::Virtex7 => {
                    for attr in [
                        "AFFINIT", "BFFINIT", "CFFINIT", "DFFINIT", "A5FFINIT", "B5FFINIT",
                        "C5FFINIT", "D5FFINIT",
                    ] {
                        ctx.collect_enum_bool(tile, bel, attr, "INIT0", "INIT1");
                    }
                    for attr in [
                        "AFFSR", "BFFSR", "CFFSR", "DFFSR", "A5FFSR", "B5FFSR", "C5FFSR", "D5FFSR",
                    ] {
                        ctx.collect_enum_bool(tile, bel, attr, "SRLOW", "SRHIGH");
                    }
                }
                Mode::Spartan6 => {
                    for attr in [
                        "AFFSRINIT",
                        "BFFSRINIT",
                        "CFFSRINIT",
                        "DFFSRINIT",
                        "A5FFSRINIT",
                        "B5FFSRINIT",
                        "C5FFSRINIT",
                        "D5FFSRINIT",
                    ] {
                        ctx.collect_enum_bool(tile, bel, attr, "SRINIT0", "SRINIT1");
                    }
                }
            }
        }
    }
}
