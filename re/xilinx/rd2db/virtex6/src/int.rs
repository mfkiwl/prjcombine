use prjcombine_interconnect::{
    db::{IntDb, WireKind},
    dir::Dir,
};
use prjcombine_re_xilinx_rawdump::{Coord, Part};

use prjcombine_re_xilinx_naming::db::NamingDb;
use prjcombine_re_xilinx_rd2db_interconnect::IntBuilder;
use prjcombine_virtex4::{bels, cslots, regions, tslots};

pub fn make_int_db(rd: &Part) -> (IntDb, NamingDb) {
    let mut builder = IntBuilder::new(
        rd,
        IntDb::new(tslots::SLOTS, bels::SLOTS, regions::SLOTS, cslots::SLOTS),
    );

    for slot in [
        bels::BSCAN0,
        bels::BSCAN1,
        bels::BSCAN2,
        bels::BSCAN3,
        bels::ICAP0,
        bels::ICAP1,
        bels::STARTUP,
        bels::CAPTURE,
        bels::JTAGPPC,
        bels::PMV0,
        bels::PMV1,
        bels::DCIRESET,
        bels::FRAME_ECC,
        bels::USR_ACCESS,
        bels::DNA_PORT,
        bels::KEY_CLEAR,
        bels::EFUSE_USR,
        bels::CFG_IO_ACCESS,
        bels::SYSMON,
        bels::IPAD_VP,
        bels::IPAD_VN,
    ] {
        // Virtex 6 special: CFG is BEL_B because it's shared with CMT.
        builder.db.bel_slots[slot].tile_slot = tslots::BEL_B;
    }

    builder.wire("GND", WireKind::Tie0, &["GND_WIRE"]);
    builder.wire("VCC", WireKind::Tie1, &["VCC_WIRE"]);

    for i in 0..8 {
        builder.wire(
            format!("LCLK{i}"),
            WireKind::Regional(regions::LEAF),
            &[format!("GCLK_B{i}")],
        );
    }

    for (lr, dir, dbeg, dend) in [
        ("L", Dir::E, Some((3, Dir::N)), Some((0, Dir::S, 3))),
        ("R", Dir::E, Some((0, Dir::S)), Some((3, Dir::N, 3))),
        ("L", Dir::W, Some((3, Dir::N)), Some((3, Dir::N, 1))),
        ("R", Dir::W, Some((0, Dir::S)), Some((0, Dir::S, 1))),
        ("L", Dir::N, Some((3, Dir::N)), Some((0, Dir::S, 3))),
        ("R", Dir::N, None, None),
        ("L", Dir::S, None, None),
        ("R", Dir::S, Some((0, Dir::S)), Some((3, Dir::N, 3))),
    ] {
        for i in 0..4 {
            let beg;
            if let Some((xi, dbeg)) = dbeg {
                if xi == i {
                    let beg_x = builder.mux_out(
                        format!("SNG.{dir}{lr}{i}.0.{dbeg}"),
                        &[format!("{dir}{lr}1BEG_{dbeg}{i}")],
                    );
                    if dir == dbeg {
                        continue;
                    }
                    beg = builder.branch(
                        beg_x,
                        !dbeg,
                        format!("SNG.{dir}{lr}{i}.0"),
                        &[format!("{dir}{lr}1BEG{i}")],
                    );
                } else {
                    beg = builder.mux_out(
                        format!("SNG.{dir}{lr}{i}.0"),
                        &[format!("{dir}{lr}1BEG{i}")],
                    );
                }
            } else {
                beg = builder.mux_out(
                    format!("SNG.{dir}{lr}{i}.0"),
                    &[format!("{dir}{lr}1BEG{i}")],
                );
            }
            let end = builder.branch(
                beg,
                dir,
                format!("SNG.{dir}{lr}{i}.1"),
                &[format!("{dir}{lr}1END{i}")],
            );
            if let Some((xi, dend, n)) = dend
                && i == xi
            {
                builder.branch(
                    end,
                    dend,
                    format!("SNG.{dir}{lr}{i}.2"),
                    &[format!("{dir}{lr}1END_{dend}{n}_{i}")],
                );
            }
        }
    }

    for (da, db, dend) in [
        (Dir::E, Dir::E, None),
        (Dir::W, Dir::W, Some((3, Dir::N, 0))),
        (Dir::N, Dir::N, Some((0, Dir::S, 2))),
        (Dir::N, Dir::E, Some((0, Dir::S, 3))),
        (Dir::N, Dir::W, Some((0, Dir::S, 0))),
        (Dir::S, Dir::S, Some((3, Dir::N, 0))),
        (Dir::S, Dir::E, None),
        (Dir::S, Dir::W, Some((3, Dir::N, 0))),
    ] {
        for i in 0..4 {
            let b = builder.mux_out(format!("DBL.{da}{db}{i}.0"), &[format!("{da}{db}2BEG{i}")]);
            let m = builder.branch(
                b,
                da,
                format!("DBL.{da}{db}{i}.1"),
                &[format!("{da}{db}2A{i}")],
            );
            let e = builder.branch(
                m,
                db,
                format!("DBL.{da}{db}{i}.2"),
                &[format!("{da}{db}2END{i}")],
            );
            if let Some((xi, dend, n)) = dend
                && i == xi
            {
                builder.branch(
                    e,
                    dend,
                    format!("DBL.{da}{db}{i}.3"),
                    &[format!("{da}{db}2END_{dend}{n}_{i}")],
                );
            }
        }
    }

    for (da, db, dend) in [
        (Dir::E, Dir::E, None),
        (Dir::W, Dir::W, Some((0, Dir::S, 0))),
        (Dir::N, Dir::N, Some((0, Dir::S, 1))),
        (Dir::N, Dir::E, None),
        (Dir::N, Dir::W, Some((0, Dir::S, 0))),
        (Dir::S, Dir::S, Some((3, Dir::N, 0))),
        (Dir::S, Dir::E, None),
        (Dir::S, Dir::W, Some((3, Dir::N, 0))),
    ] {
        for i in 0..4 {
            let b = builder.mux_out(format!("QUAD.{da}{db}{i}.0"), &[format!("{da}{db}4BEG{i}")]);
            let a = builder.branch(
                b,
                db,
                format!("QUAD.{da}{db}{i}.1"),
                &[format!("{da}{db}4A{i}")],
            );
            let m = builder.branch(
                a,
                da,
                format!("QUAD.{da}{db}{i}.2"),
                &[format!("{da}{db}4B{i}")],
            );
            let c = builder.branch(
                m,
                da,
                format!("QUAD.{da}{db}{i}.3"),
                &[format!("{da}{db}4C{i}")],
            );
            let e = builder.branch(
                c,
                db,
                format!("QUAD.{da}{db}{i}.4"),
                &[format!("{da}{db}4END{i}")],
            );
            if let Some((xi, dend, n)) = dend
                && i == xi
            {
                builder.branch(
                    e,
                    dend,
                    format!("QUAD.{da}{db}{i}.5"),
                    &[format!("{da}{db}4END_{dend}{n}_{i}")],
                );
            }
        }
    }

    // The long wires.
    let mid = builder.wire("LH.8", WireKind::MultiOut, &["LH8"]);
    let mut prev = mid;
    for i in (0..8).rev() {
        prev = builder.multi_branch(prev, Dir::E, format!("LH.{i}"), &[format!("LH{i}")]);
    }
    let mut prev = mid;
    for i in 9..17 {
        prev = builder.multi_branch(prev, Dir::W, format!("LH.{i}"), &[format!("LH{i}")]);
    }
    let mid = builder.wire("LV.8", WireKind::MultiOut, &["LV8"]);
    let mut prev = mid;
    let mut lv_bh_n = Vec::new();
    for i in (0..8).rev() {
        prev = builder.multi_branch(prev, Dir::S, format!("LV.{i}"), &[format!("LV{i}")]);
        lv_bh_n.push(prev);
    }
    let mut prev = mid;
    let mut lv_bh_s = Vec::new();
    for i in 9..17 {
        prev = builder.multi_branch(prev, Dir::N, format!("LV.{i}"), &[format!("LV{i}")]);
        lv_bh_s.push(prev);
    }

    // The control inputs.
    for i in 0..2 {
        builder.mux_out(format!("IMUX.GFAN{i}"), &[format!("GFAN{i}")]);
    }
    for i in 0..2 {
        builder.mux_out(format!("IMUX.CLK{i}"), &[format!("CLK_B{i}")]);
    }
    for i in 0..2 {
        builder.mux_out(format!("IMUX.CTRL{i}"), &[format!("CTRL_B{i}")]);
    }
    for i in 0..8 {
        builder.mux_out(format!("IMUX.BYP{i}"), &[format!("BYP{i}")]);
        builder.permabuf(format!("IMUX.BYP{i}.SITE"), &[format!("BYP_B{i}")]);
        let b = builder.permabuf(format!("IMUX.BYP{i}.BOUNCE"), &[format!("BYP_BOUNCE{i}")]);
        if matches!(i, 2 | 3 | 6 | 7) {
            builder.branch(
                b,
                Dir::N,
                format!("IMUX.BYP{i}.BOUNCE.N"),
                &[format!("BYP_BOUNCE_N3_{i}")],
            );
        }
    }
    for i in 0..8 {
        builder.mux_out(format!("IMUX.FAN{i}"), &[format!("FAN{i}")]);
        builder.permabuf(format!("IMUX.FAN{i}.SITE"), &[format!("FAN_B{i}")]);
        let b = builder.permabuf(format!("IMUX.FAN{i}.BOUNCE"), &[format!("FAN_BOUNCE{i}")]);
        if matches!(i, 0 | 2 | 4 | 6) {
            builder.branch(
                b,
                Dir::S,
                format!("IMUX.FAN{i}.BOUNCE.S"),
                &[format!("FAN_BOUNCE_S3_{i}")],
            );
        }
    }
    for i in 0..48 {
        let w = builder.mux_out(format!("IMUX.IMUX{i}"), &[format!("IMUX_B{i}")]);
        builder.delay(w, format!("IMUX.IMUX{i}.DELAY"), &[""]);
    }

    for i in 0..24 {
        builder.logic_out(format!("OUT{i}"), &[format!("LOGIC_OUTS{i}")]);
    }

    for i in 0..4 {
        builder.test_out(
            format!("TEST{i}"),
            &[
                format!("INT_INTERFACE_BLOCK_OUTS_B{i}"),
                format!("EMAC_INT_INTERFACE_BLOCK_OUTS_B{i}"),
                format!("PCIE_INT_INTERFACE_BLOCK_OUTS_B{i}"),
                format!("PCIE_INT_INTERFACE_L_BLOCK_OUTS_B{i}"),
                format!("IOI_L_INT_INTERFACE_BLOCK_OUTS_B{i}"),
                format!("GTX_INT_INTERFACE_BLOCK_OUTS_B{i}"),
                format!("GT_L_INT_INTERFACE_BLOCK_OUTS_B{i}"),
            ],
        );
    }

    builder.extract_main_passes();

    builder.node_type(tslots::INT, bels::INT, "INT", "INT", "INT");

    builder.extract_term_conn("TERM.W", Dir::W, "L_TERM_INT", &[]);
    builder.extract_term_conn("TERM.E", Dir::E, "R_TERM_INT", &[]);
    builder.extract_term_conn("TERM.S", Dir::S, "BRKH_T_TERM_INT", &[]);
    for &xy in rd.tiles_by_kind_name("PCIE") {
        let int_xy_a = Coord {
            x: xy.x,
            y: xy.y + 11,
        };
        let int_xy_b = Coord {
            x: xy.x + 2,
            y: xy.y + 11,
        };
        builder.extract_term_conn_tile("TERM.S", Dir::S, int_xy_a, &[]);
        builder.extract_term_conn_tile("TERM.S", Dir::S, int_xy_b, &[]);
    }
    builder.extract_term_conn("TERM.N", Dir::N, "BRKH_B_TERM_INT", &[]);
    builder.make_blackhole_term("TERM.S.HOLE", Dir::S, &lv_bh_s);
    builder.make_blackhole_term("TERM.N.HOLE", Dir::N, &lv_bh_n);

    builder.extract_intf(
        tslots::INTF,
        "INTF",
        Dir::E,
        "INT_INTERFACE",
        "INTF",
        true,
        None,
    );
    builder.extract_intf(
        tslots::INTF,
        "INTF",
        Dir::E,
        "IOI_L_INT_INTERFACE",
        "INTF.IOI_L",
        true,
        None,
    );
    for (n, tkn) in [
        ("GT_L", "GT_L_INT_INTERFACE"),
        ("GTX", "GTX_INT_INTERFACE"),
        ("EMAC", "EMAC_INT_INTERFACE"),
        ("PCIE_L", "PCIE_INT_INTERFACE_L"),
        ("PCIE_R", "PCIE_INT_INTERFACE_R"),
    ] {
        builder.extract_intf(
            tslots::INTF,
            "INTF.DELAY",
            Dir::E,
            tkn,
            format!("INTF.{n}"),
            true,
            Some(bels::INTF_DELAY),
        );
    }

    for tkn in ["CLBLL", "CLBLM"] {
        if let Some(&xy) = rd.tiles_by_kind_name(tkn).iter().next() {
            let int_xy = Coord {
                x: xy.x - 1,
                y: xy.y,
            };
            builder.extract_xnode_bels(
                tslots::BEL,
                tkn,
                xy,
                &[],
                &[int_xy],
                tkn,
                &[
                    builder
                        .bel_xy(bels::SLICE0, "SLICE", 0, 0)
                        .pin_name_only("CIN", 0)
                        .pin_name_only("COUT", 1),
                    builder
                        .bel_xy(bels::SLICE1, "SLICE", 1, 0)
                        .pin_name_only("CIN", 0)
                        .pin_name_only("COUT", 1),
                ],
            );
        }
    }

    if let Some(&xy) = rd.tiles_by_kind_name("BRAM").iter().next() {
        let mut int_xy = Vec::new();
        let mut intf_xy = Vec::new();
        let n = builder.ndb.get_tile_class_naming("INTF");
        for dy in 0..5 {
            int_xy.push(Coord {
                x: xy.x - 2,
                y: xy.y + dy,
            });
            intf_xy.push((
                Coord {
                    x: xy.x - 1,
                    y: xy.y + dy,
                },
                n,
            ));
        }
        let bel_bram_f = builder
            .bel_xy(bels::BRAM_F, "RAMB36", 0, 0)
            .pins_name_only(&[
                "CASCADEINA",
                "CASCADEINB",
                "TSTOUT1",
                "TSTOUT2",
                "TSTOUT3",
                "TSTOUT4",
            ])
            .pin_name_only("CASCADEOUTA", 1)
            .pin_name_only("CASCADEOUTB", 1);
        let bel_bram_h0 = builder.bel_xy(bels::BRAM_H0, "RAMB18", 0, 0);
        let mut bel_bram_h1 = builder
            .bel_xy(bels::BRAM_H1, "RAMB18", 0, 1)
            .pins_name_only(&[
                "FULL",
                "EMPTY",
                "ALMOSTFULL",
                "ALMOSTEMPTY",
                "WRERR",
                "RDERR",
            ]);
        for i in 0..12 {
            bel_bram_h1 = bel_bram_h1.pin_name_only(&format!("RDCOUNT{i}"), 0);
            bel_bram_h1 = bel_bram_h1.pin_name_only(&format!("WRCOUNT{i}"), 0);
        }
        builder.extract_xnode_bels_intf(
            tslots::BEL,
            "BRAM",
            xy,
            &[],
            &int_xy,
            &intf_xy,
            "BRAM",
            &[bel_bram_f, bel_bram_h0, bel_bram_h1],
        );
    }

    if let Some(&xy) = rd.tiles_by_kind_name("HCLK_BRAM").iter().next() {
        let mut int_xy = Vec::new();
        let mut intf_xy = Vec::new();
        let n = builder.ndb.get_tile_class_naming("INTF");
        for dy in 0..15 {
            int_xy.push(Coord {
                x: xy.x - 2,
                y: xy.y + 1 + dy,
            });
            intf_xy.push((
                Coord {
                    x: xy.x - 1,
                    y: xy.y + 1 + dy,
                },
                n,
            ));
        }
        let mut bram_xy = Vec::new();
        for dy in [1, 6, 11] {
            bram_xy.push(Coord {
                x: xy.x,
                y: xy.y + dy,
            });
        }
        builder.extract_xnode_bels_intf(
            tslots::HCLK_BEL,
            "PMVBRAM",
            xy,
            &bram_xy,
            &int_xy,
            &intf_xy,
            "PMVBRAM",
            &[builder.bel_xy(bels::PMVBRAM, "PMVBRAM", 0, 0)],
        );
    }

    if let Some(&xy) = rd.tiles_by_kind_name("DSP").iter().next() {
        let mut int_xy = Vec::new();
        let mut intf_xy = Vec::new();
        let n = builder.ndb.get_tile_class_naming("INTF");
        for dy in 0..5 {
            int_xy.push(Coord {
                x: xy.x - 2,
                y: xy.y + dy,
            });
            intf_xy.push((
                Coord {
                    x: xy.x - 1,
                    y: xy.y + dy,
                },
                n,
            ));
        }

        let mut bels_dsp = vec![];
        for i in 0..2 {
            let mut bel = builder.bel_xy(bels::DSP[i], "DSP48", 0, i);
            let buf_cnt = match i {
                0 => 0,
                1 => 1,
                _ => unreachable!(),
            };
            bel = bel.pin_name_only("MULTSIGNIN", 0);
            bel = bel.pin_name_only("MULTSIGNOUT", buf_cnt);
            bel = bel.pin_name_only("CARRYCASCIN", 0);
            bel = bel.pin_name_only("CARRYCASCOUT", buf_cnt);
            for j in 0..30 {
                bel = bel.pin_name_only(&format!("ACIN{j}"), 0);
                bel = bel.pin_name_only(&format!("ACOUT{j}"), buf_cnt);
            }
            for j in 0..18 {
                bel = bel.pin_name_only(&format!("BCIN{j}"), 0);
                bel = bel.pin_name_only(&format!("BCOUT{j}"), buf_cnt);
            }
            for j in 0..48 {
                bel = bel.pin_name_only(&format!("PCIN{j}"), 0);
                bel = bel.pin_name_only(&format!("PCOUT{j}"), buf_cnt);
            }
            bels_dsp.push(bel);
        }
        bels_dsp.push(
            builder
                .bel_xy(bels::TIEOFF_DSP, "TIEOFF", 0, 0)
                .pins_name_only(&["HARD0", "HARD1"]),
        );
        builder.extract_xnode_bels_intf(
            tslots::BEL,
            "DSP",
            xy,
            &[],
            &int_xy,
            &intf_xy,
            "DSP",
            &bels_dsp,
        );
    }

    if let Some(&xy) = rd.tiles_by_kind_name("EMAC").iter().next() {
        let mut int_xy = Vec::new();
        let mut intf_xy = Vec::new();
        let n = builder.ndb.get_tile_class_naming("INTF.EMAC");
        for dy in 0..10 {
            int_xy.push(Coord {
                x: xy.x - 2,
                y: xy.y + dy,
            });
            intf_xy.push((
                Coord {
                    x: xy.x - 1,
                    y: xy.y + dy,
                },
                n,
            ));
        }
        builder.extract_xnode_bels_intf(
            tslots::BEL,
            "EMAC",
            xy,
            &[],
            &int_xy,
            &intf_xy,
            "EMAC",
            &[builder.bel_xy(bels::EMAC, "TEMAC", 0, 0)],
        );
    }

    if let Some(&xy) = rd.tiles_by_kind_name("PCIE").iter().next() {
        let mut int_xy = Vec::new();
        let mut intf_xy = Vec::new();
        let nl = builder.ndb.get_tile_class_naming("INTF.PCIE_L");
        let nr = builder.ndb.get_tile_class_naming("INTF.PCIE_R");
        for dy in 0..20 {
            int_xy.push(Coord {
                x: xy.x - 4,
                y: xy.y - 10 + dy,
            });
            intf_xy.push((
                Coord {
                    x: xy.x - 3,
                    y: xy.y - 10 + dy,
                },
                nl,
            ));
        }
        for dy in 0..20 {
            int_xy.push(Coord {
                x: xy.x - 2,
                y: xy.y - 10 + dy,
            });
            intf_xy.push((
                Coord {
                    x: xy.x - 1,
                    y: xy.y - 10 + dy,
                },
                nr,
            ));
        }
        builder.extract_xnode_bels_intf(
            tslots::BEL,
            "PCIE",
            xy,
            &[],
            &int_xy,
            &intf_xy,
            "PCIE",
            &[builder.bel_xy(bels::PCIE, "PCIE", 0, 0)],
        );
    }

    for (tkn, naming) in [
        ("HCLK", "HCLK"),
        ("HCLK_QBUF_L", "HCLK.QBUF"),
        ("HCLK_QBUF_R", "HCLK.QBUF"),
    ] {
        if let Some(&xy) = rd.tiles_by_kind_name(tkn).iter().next() {
            let bel_gsig = builder.bel_xy(bels::GLOBALSIG, "GLOBALSIG", 0, 0);
            let mut bel = builder.bel_virtual(bels::HCLK);
            for i in 0..8 {
                bel = bel
                    .extra_int_out(format!("LCLK{i}_D"), &[format!("HCLK_LEAF_CLK_B_BOT{i}")])
                    .extra_int_out(format!("LCLK{i}_U"), &[format!("HCLK_LEAF_CLK_B_TOP{i}")]);
            }
            for i in 0..12 {
                bel = bel.extra_wire(
                    format!("HCLK{i}"),
                    &[
                        format!("HCLK_CK_BUFHCLK{i}"),
                        format!("HCLK_QBUF_CK_BUFHCLK{i}"),
                    ],
                );
            }
            for i in 0..6 {
                bel = bel.extra_wire(
                    format!("RCLK{i}"),
                    &[
                        format!("HCLK_CK_BUFRCLK{i}"),
                        format!("HCLK_QBUF_CK_BUFRCLK{i}"),
                    ],
                );
            }
            builder
                .xnode(tslots::HCLK, "HCLK", naming, xy)
                .num_tiles(2)
                .ref_int(xy.delta(0, -1), 0)
                .ref_int(xy.delta(0, 1), 1)
                .bel(bel_gsig)
                .bel(bel)
                .extract();
            if naming == "HCLK.QBUF" {
                let mut bel = builder.bel_virtual(bels::HCLK_QBUF);
                for i in 0..12 {
                    bel = bel
                        .extra_wire(format!("HCLK{i}_O"), &[format!("HCLK_QBUF_CK_BUFHCLK{i}")])
                        .extra_wire(
                            format!("HCLK{i}_I"),
                            &[format!("HCLK_QBUF_CK_BUFH2QBUF{i}")],
                        );
                }
                builder
                    .xnode(tslots::HROW, "HCLK_QBUF", "HCLK_QBUF", xy)
                    .num_tiles(0)
                    .bel(bel)
                    .extract();
            }
        }
    }

    for (tkn, naming_l, naming_r) in [
        ("HCLK_INNER_IOI", "HCLK_IOI.IL", "HCLK_IOI.IR"),
        ("HCLK_OUTER_IOI", "HCLK_IOI.OL", "HCLK_IOI.OR"),
    ] {
        for &xy in rd.tiles_by_kind_name(tkn) {
            let is_l = rd.tile_kinds.key(rd.tiles[&xy.delta(-1, 0)].kind) == "HCLK_IOB";
            let hclk_xy = if is_l {
                if rd.tile_kinds.key(rd.tiles[&xy.delta(1, 0)].kind) == "HCLK_TERM" {
                    xy.delta(2, 0)
                } else {
                    xy.delta(1, 0)
                }
            } else {
                if rd.tile_kinds.key(rd.tiles[&xy.delta(-1, 0)].kind) == "HCLK_TERM" {
                    xy.delta(-3, 0)
                } else {
                    xy.delta(-2, 0)
                }
            };
            let intf_io =
                builder
                    .ndb
                    .get_tile_class_naming(if is_l { "INTF.IOI_L" } else { "INTF" });
            let mut bels = vec![];
            for i in 0..4 {
                bels.push(
                    builder
                        .bel_xy(bels::BUFIO[i], "BUFIODQS", 0, i ^ 2)
                        .pins_name_only(&["I", "O"]),
                );
            }
            for i in 0..2 {
                bels.push(
                    builder
                        .bel_xy(bels::BUFR[i], "BUFR", 0, i ^ 1)
                        .pins_name_only(&["I", "O"]),
                );
            }
            for i in 0..2 {
                bels.push(
                    builder
                        .bel_xy(bels::BUFO[i], "BUFO", 0, i ^ 1)
                        .pins_name_only(&["I", "O"])
                        .extra_wire("VI", &[format!("HCLK_IOI_VBUFOCLK{i}")])
                        .extra_wire("VI_S", &[format!("HCLK_IOI_VBUFOCLK_SOUTH{i}")])
                        .extra_wire("VI_N", &[format!("HCLK_IOI_VBUFOCLK_NORTH{i}")])
                        .extra_wire("I_PRE", &[format!("HCLK_IOI_BUFOCLK{i}")])
                        .extra_wire("I_PRE2", &[format!("HCLK_IOI_CLKB_TO_BUFO{i}")]),
                );
            }
            bels.push(
                builder
                    .bel_xy(bels::IDELAYCTRL, "IDELAYCTRL", 0, 0)
                    .pins_name_only(&["REFCLK"]),
            );
            bels.push(builder.bel_xy(bels::DCI, "DCI", 0, 0).pins_name_only(&[
                "DCIDATA",
                "DCIADDRESS0",
                "DCIADDRESS1",
                "DCIADDRESS2",
                "DCIIOUPDATE",
                "DCIREFIOUPDATE",
                "DCISCLK",
            ]));
            let mut bel = builder
                .bel_virtual(bels::HCLK_IOI)
                .extra_int_in("BUFR_CKINT0", &["HCLK_IOI_RCLK_IMUX_BOT_B"])
                .extra_int_in("BUFR_CKINT1", &["HCLK_IOI_RCLK_IMUX_TOP_B"]);
            for i in 0..12 {
                bel = bel
                    .extra_wire(format!("HCLK{i}_O"), &[format!("HCLK_IOI_LEAF_GCLK{i}")])
                    .extra_wire(format!("HCLK{i}_I"), &[format!("HCLK_IOI_CK_BUFHCLK{i}")]);
            }
            for i in 0..6 {
                bel = bel
                    .extra_wire(format!("RCLK{i}_O"), &[format!("HCLK_IOI_RCLK_TO_IO{i}")])
                    .extra_wire(format!("RCLK{i}_I"), &[format!("HCLK_IOI_CK_BUFRCLK{i}")]);
            }
            for i in 0..2 {
                bel = bel.extra_wire(format!("OCLK{i}"), &[format!("HCLK_IOI_OCLK{i}")]);
            }
            for i in 0..2 {
                bel = bel.extra_wire(format!("VRCLK{i}"), &[format!("HCLK_IOI_VRCLK{i}")]);
                bel = bel.extra_wire(format!("VRCLK{i}_S"), &[format!("HCLK_IOI_VRCLK_SOUTH{i}")]);
                bel = bel.extra_wire(format!("VRCLK{i}_N"), &[format!("HCLK_IOI_VRCLK_NORTH{i}")]);
            }
            for i in 0..4 {
                bel = bel
                    .extra_wire(
                        format!("PERF{i}"),
                        &[if tkn == "HCLK_INNER_IOI" {
                            format!("HCLK_IOI_CK_PERF_INNER{i}")
                        } else {
                            format!("HCLK_IOI_CK_PERF_OUTER{i}")
                        }],
                    )
                    .extra_wire(
                        format!("PERF{i}_BUF"),
                        &[format!("HCLK_IOI_IO_PLL_CLK{ii}_BUFF", ii = i ^ 1)],
                    )
                    .extra_wire(
                        format!("IOCLK_IN{i}"),
                        &[format!("HCLK_IOI_IO_PLL_CLK{i}_DMUX")],
                    )
                    .extra_wire(
                        format!("IOCLK_IN{i}_BUFR"),
                        &[if i < 2 {
                            format!("HCLK_IOI_RCLK_TOP{i}")
                        } else {
                            format!("HCLK_IOI_RCLK_BOT{ii}", ii = i - 2)
                        }],
                    )
                    .extra_wire(
                        format!("IOCLK_PAD{i}"),
                        &[if i < 2 {
                            format!("HCLK_IOI_I2IOCLK_TOP{i}")
                        } else {
                            format!("HCLK_IOI_I2IOCLK_BOT{ii}", ii = i - 2)
                        }],
                    );
            }
            for i in 0..4 {
                bel = bel
                    .extra_wire(format!("IOCLK{i}"), &[format!("HCLK_IOI_IOCLK{i}")])
                    .extra_wire(
                        format!("IOCLK{ii}", ii = i + 4),
                        &[format!("HCLK_IOI_IOCLKMULTI{i}")],
                    )
                    .extra_wire(format!("IOCLK{i}_DLY"), &[format!("HCLK_IOI_IOCLK{i}_DLY")])
                    .extra_wire(
                        format!("IOCLK{ii}_DLY", ii = i + 4),
                        &[format!("HCLK_IOI_IOCLKMULTI{i}_DLY")],
                    );
            }
            bel = bel
                .extra_wire("IOCLK0_PRE", &["HCLK_IOI_VIOCLK0"])
                .extra_wire("IOCLK1_PRE", &["HCLK_IOI_SIOCLK1"])
                .extra_wire("IOCLK2_PRE", &["HCLK_IOI_SIOCLK2"])
                .extra_wire("IOCLK3_PRE", &["HCLK_IOI_VIOCLK1"])
                .extra_wire("IOCLK0_PRE_S", &["HCLK_IOI_VIOCLK_SOUTH0"])
                .extra_wire("IOCLK3_PRE_S", &["HCLK_IOI_VIOCLK_SOUTH1"])
                .extra_wire("IOCLK0_PRE_N", &["HCLK_IOI_VIOCLK_NORTH0"])
                .extra_wire("IOCLK3_PRE_N", &["HCLK_IOI_VIOCLK_NORTH1"]);
            for i in 0..10 {
                bel = bel.extra_wire(format!("MGT{i}"), &[format!("HCLK_IOI_CK_MGT{i}")]);
            }
            bels.push(bel);
            builder
                .xnode(
                    tslots::HCLK_BEL,
                    "HCLK_IOI",
                    if is_l { naming_l } else { naming_r },
                    xy,
                )
                .raw_tile(xy.delta(0, -2))
                .raw_tile(xy.delta(0, 1))
                .num_tiles(2)
                .ref_int(hclk_xy.delta(0, -1), 0)
                .ref_int(hclk_xy.delta(0, 1), 1)
                .ref_single(hclk_xy.delta(1, -1), 0, intf_io)
                .ref_single(hclk_xy.delta(1, 1), 1, intf_io)
                .bels(bels)
                .extract();
        }
    }

    for tkn in ["LIOI", "RIOI"] {
        if let Some(&xy) = rd.tiles_by_kind_name(tkn).iter().next() {
            let is_l = tkn == "LIOI";
            let lr = if is_l { 'L' } else { 'R' };
            let int_xy = if is_l {
                builder.walk_to_int(xy, Dir::E, false).unwrap()
            } else {
                builder.walk_to_int(xy, Dir::W, false).unwrap()
            };
            let intf_io =
                builder
                    .ndb
                    .get_tile_class_naming(if is_l { "INTF.IOI_L" } else { "INTF" });
            let mut bels = vec![];
            for i in 0..2 {
                let ii = i ^ 1;
                let mut bel = builder
                    .bel_xy(bels::ILOGIC[i], "ILOGIC", 0, i)
                    .pins_name_only(&[
                        "CLK",
                        "CLKB",
                        "OCLK",
                        "OCLKB",
                        "D",
                        "DDLY",
                        "OFB",
                        "TFB",
                        "SHIFTIN1",
                        "SHIFTIN2",
                        "SHIFTOUT1",
                        "SHIFTOUT2",
                        "REV",
                    ])
                    .extra_wire(
                        "IOB_I",
                        &[format!("LIOI_IBUF{ii}"), format!("RIOI_IBUF{ii}")],
                    )
                    .extra_wire("IOB_I_BUF", &[format!("LIOI_I{ii}"), format!("RIOI_I{ii}")])
                    .extra_int_in("CKINT0", &[format!("IOI_IMUX_B14_{i}")])
                    .extra_int_in("CKINT1", &[format!("IOI_IMUX_B15_{i}")]);
                if i == 1 {
                    bel = bel
                        .extra_wire_force("CLKOUT", format!("{lr}IOI_I_2IOCLK_BOT1"))
                        .extra_wire_force("CLKOUT_CMT", format!("{lr}IOI_I_2IOCLK_BOT1_I2GCLK"));
                }
                bels.push(bel);
            }
            for i in 0..2 {
                let ii = i ^ 1;
                bels.push(
                    builder
                        .bel_xy(bels::OLOGIC[i], "OLOGIC", 0, i)
                        .pins_name_only(&[
                            "CLK",
                            "CLKB",
                            "CLKDIVB",
                            "CLKPERF",
                            "CLKPERFDELAY",
                            "OFB",
                            "TFB",
                            "TQ",
                            "OQ",
                            "SHIFTIN1",
                            "SHIFTIN2",
                            "SHIFTOUT1",
                            "SHIFTOUT2",
                            "REV",
                        ])
                        .extra_int_out(
                            "CLKDIV",
                            &[
                                format!("LIOI_OLOGIC{ii}_CLKDIV"),
                                format!("RIOI_OLOGIC{ii}_CLKDIV"),
                            ],
                        )
                        .extra_int_in("CLKDIV_CKINT", &[format!("IOI_IMUX_B20_{i}")])
                        .extra_int_in("CLK_CKINT", &[format!("IOI_IMUX_B21_{i}")])
                        .extra_int_out("CLK_MUX", &[format!("IOI_OCLK_{ii}")])
                        .extra_wire("CLKM", &[format!("IOI_OCLKM_{ii}")])
                        .extra_int_out(
                            "TFB_BUF",
                            &[
                                format!("LIOI_OLOGIC{ii}_TFB_LOCAL"),
                                format!("RIOI_OLOGIC{ii}_TFB_LOCAL"),
                            ],
                        )
                        .extra_wire("IOB_O", &[format!("LIOI_O{ii}"), format!("RIOI_O{ii}")])
                        .extra_wire("IOB_T", &[format!("LIOI_T{ii}"), format!("RIOI_T{ii}")]),
                );
            }
            for i in 0..2 {
                bels.push(
                    builder
                        .bel_xy(bels::IODELAY[i], "IODELAY", 0, i)
                        .pins_name_only(&["CLKIN", "IDATAIN", "ODATAIN", "DATAOUT", "T"]),
                );
            }
            for i in 0..2 {
                let mut bel = builder
                    .bel_xy(bels::IOB[i], "IOB", 0, i)
                    .raw_tile(1)
                    .pins_name_only(&[
                        "I",
                        "O",
                        "T",
                        "PADOUT",
                        "DIFFI_IN",
                        "DIFFO_OUT",
                        "DIFFO_IN",
                        "O_OUT",
                        "O_IN",
                    ]);
                if i == 1 {
                    bel = bel.pins_name_only(&["DIFF_TERM_INT_EN"]);
                }
                let pn = if i == 1 { 'P' } else { 'N' };
                bel = bel.extra_wire_force("MONITOR", format!("{lr}IOB_MONITOR_{pn}"));
                bels.push(bel);
            }
            let mut bel = builder.bel_virtual(bels::IOI);
            for i in 0..2 {
                bel = bel.extra_wire(format!("OCLK{i}"), &[format!("IOI_BUFO_CLK{i}")])
            }
            for i in 0..8 {
                bel = bel.extra_wire(format!("IOCLK{i}"), &[format!("IOI_IOCLK{i}")])
            }
            for i in 0..12 {
                bel = bel.extra_wire(format!("HCLK{i}"), &[format!("IOI_LEAF_GCLK{i}")])
            }
            for i in 0..6 {
                bel = bel.extra_wire(format!("RCLK{i}"), &[format!("IOI_RCLK_FORIO{i}")])
            }
            bels.push(bel);
            builder
                .xnode(tslots::BEL, "IO", tkn, xy)
                .raw_tile(if is_l {
                    xy.delta(-1, 0)
                } else {
                    xy.delta(1, 0)
                })
                .num_tiles(2)
                .ref_int(int_xy, 0)
                .ref_int(int_xy.delta(0, 1), 1)
                .ref_single(int_xy.delta(1, 0), 0, intf_io)
                .ref_single(int_xy.delta(1, 1), 1, intf_io)
                .bels(bels)
                .extract();
        }
    }

    if let Some(&xy) = rd.tiles_by_kind_name("CFG_CENTER_0").iter().next() {
        let intf = builder.ndb.get_tile_class_naming("INTF");
        let mut bel_sysmon = builder
            .bel_xy(bels::SYSMON, "SYSMON", 0, 0)
            .raw_tile(2)
            .pins_name_only(&["VP", "VN"]);
        for i in 0..16 {
            bel_sysmon = bel_sysmon
                .pin_name_only(&format!("VAUXP{i}"), 1)
                .pin_name_only(&format!("VAUXN{i}"), 1);
        }
        let bels = [
            builder.bel_xy(bels::BSCAN0, "BSCAN", 0, 0).raw_tile(1),
            builder.bel_xy(bels::BSCAN1, "BSCAN", 0, 1).raw_tile(1),
            builder.bel_xy(bels::BSCAN2, "BSCAN", 0, 0).raw_tile(2),
            builder.bel_xy(bels::BSCAN3, "BSCAN", 0, 1).raw_tile(2),
            builder.bel_xy(bels::ICAP0, "ICAP", 0, 0).raw_tile(1),
            builder.bel_xy(bels::ICAP1, "ICAP", 0, 0).raw_tile(2),
            builder.bel_xy(bels::PMV0, "PMV", 0, 0).raw_tile(0),
            builder.bel_xy(bels::PMV1, "PMV", 0, 0).raw_tile(3),
            builder.bel_xy(bels::STARTUP, "STARTUP", 0, 0).raw_tile(1),
            builder.bel_xy(bels::CAPTURE, "CAPTURE", 0, 0).raw_tile(1),
            builder.bel_single(bels::FRAME_ECC, "FRAME_ECC").raw_tile(1),
            builder
                .bel_xy(bels::EFUSE_USR, "EFUSE_USR", 0, 0)
                .raw_tile(1),
            builder
                .bel_xy(bels::USR_ACCESS, "USR_ACCESS", 0, 0)
                .raw_tile(1),
            builder.bel_xy(bels::DNA_PORT, "DNA_PORT", 0, 0).raw_tile(1),
            builder.bel_xy(bels::DCIRESET, "DCIRESET", 0, 0).raw_tile(1),
            builder
                .bel_xy(bels::CFG_IO_ACCESS, "CFG_IO_ACCESS", 0, 0)
                .raw_tile(1),
            bel_sysmon,
            builder
                .bel_xy(bels::IPAD_VP, "IPAD", 0, 0)
                .raw_tile(2)
                .pins_name_only(&["O"]),
            builder
                .bel_xy(bels::IPAD_VN, "IPAD", 0, 1)
                .raw_tile(2)
                .pins_name_only(&["O"]),
        ];
        let mut xn = builder
            .xnode(tslots::BEL_B, "CFG", "CFG", xy)
            .num_tiles(80)
            .raw_tile(xy.delta(0, 21))
            .raw_tile(xy.delta(0, 42))
            .raw_tile(xy.delta(0, 63));
        for i in 0..80 {
            let int_xy = xy.delta(2, -10 + (i + i / 20) as i32);
            xn = xn
                .ref_int(int_xy, i)
                .ref_single(int_xy.delta(1, 0), i, intf);
        }
        xn.bels(bels).extract();
    }

    for (tkn, naming) in [("HCLK_CMT_BOT", "CMT.BOT"), ("HCLK_CMT_TOP", "CMT.TOP")] {
        if let Some(&xy) = rd.tiles_by_kind_name(tkn).iter().next() {
            let intf = builder.ndb.get_tile_class_naming("INTF");
            let xy_bot = xy.delta(0, -9);
            let xy_top = xy.delta(0, 10);
            let mut bels = vec![];
            for i in 0..2 {
                let slots = [bels::BUFHCE_W, bels::BUFHCE_E][i];
                for j in 0..12 {
                    bels.push(
                        builder
                            .bel_xy(slots[j], "BUFHCE", i, j)
                            .raw_tile(2)
                            .pins_name_only(&["I", "O"]),
                    );
                }
            }
            for i in 0..2 {
                let mut bel = builder
                    .bel_xy(bels::MMCM[i], "MMCM_ADV", 0, 0)
                    .raw_tile(i)
                    .pins_name_only(&[
                        "CLKIN1",
                        "CLKIN2",
                        "CLKFBIN",
                        "CLKOUT0",
                        "CLKOUT0B",
                        "CLKOUT1",
                        "CLKOUT1B",
                        "CLKOUT2",
                        "CLKOUT2B",
                        "CLKOUT3",
                        "CLKOUT3B",
                        "CLKOUT4",
                        "CLKOUT5",
                        "CLKOUT6",
                        "CLKFBOUT",
                        "CLKFBOUTB",
                        "TMUXOUT",
                    ])
                    .extra_wire("CLKIN1_HCLK", &["CMT_CLKIN1_HCLK"])
                    .extra_wire("CLKIN1_IO", &["CMT_CLKIN1_IO"])
                    .extra_wire("CLKIN1_MGT", &["CMT_CLKIN1_MGT"])
                    .extra_int_in("CLKIN1_CKINT", &["CMT_MMCM_IMUX_CLKIN1"])
                    .extra_wire("CLKIN2_HCLK", &["CMT_CLKIN2_HCLK"])
                    .extra_wire("CLKIN2_IO", &["CMT_CLKIN2_IO"])
                    .extra_wire("CLKIN2_MGT", &["CMT_CLKIN2_MGT"])
                    .extra_int_in("CLKIN2_CKINT", &["CMT_MMCM_IMUX_CLKIN2"])
                    .extra_wire("CLKFBIN_HCLK", &["CMT_CLKFB_HCLK"])
                    .extra_wire("CLKFBIN_IO", &["CMT_CLKFB_IO"])
                    .extra_int_in("CLKFBIN_CKINT", &["CMT_MMCM_IMUX_CLKFB"])
                    .extra_wire("CLKFB", &["CMT_MMCM_CLKFB"])
                    .extra_wire("CASC_IN", &["CMT_MMCM_CASC_IN"])
                    .extra_wire("CASC_OUT", &["CMT_MMCM_CASC_OUT"]);
                for i in 0..14 {
                    bel = bel.extra_wire(format!("CMT_OUT{i}"), &[format!("CMT_CK_MMCM_{i}")]);
                }
                for i in 0..4 {
                    bel = bel
                        .extra_wire(format!("PERF{i}"), &[format!("CMT_PERF_CLK_BOUNCE{i}")])
                        .extra_wire(format!("PERF{i}_OL"), &[format!("CMT_CK_PERF_OUTER_L{i}")])
                        .extra_wire(format!("PERF{i}_IL"), &[format!("CMT_CK_PERF_INNER_L{i}")])
                        .extra_wire(format!("PERF{i}_IR"), &[format!("CMT_CK_PERF_INNER_R{i}")])
                        .extra_wire(format!("PERF{i}_OR"), &[format!("CMT_CK_PERF_OUTER_R{i}")]);
                }
                bels.push(bel);
            }
            bels.push(
                builder
                    .bel_xy(bels::PPR_FRAME, "PPR_FRAME", 0, 0)
                    .raw_tile(1),
            );
            let mut bel = builder
                .bel_virtual(bels::CMT)
                .raw_tile(2)
                .extra_wire("BUFH_TEST_L_PRE", &["HCLK_CMT_CK_BUFH_TEST_OUT_L"])
                .extra_wire("BUFH_TEST_L_INV", &["HCLK_CMT_CK_BUFH_TEST_INV_L"])
                .extra_wire("BUFH_TEST_L_NOINV", &["HCLK_CMT_CK_BUFH_TEST_NOINV_L"])
                .extra_wire("BUFH_TEST_L", &["HCLK_CMT_CK_BUFH_TEST_L"])
                .extra_wire("BUFH_TEST_R_PRE", &["HCLK_CMT_CK_BUFH_TEST_OUT_R"])
                .extra_wire("BUFH_TEST_R_INV", &["HCLK_CMT_CK_BUFH_TEST_INV_R"])
                .extra_wire("BUFH_TEST_R_NOINV", &["HCLK_CMT_CK_BUFH_TEST_NOINV_R"])
                .extra_wire("BUFH_TEST_R", &["HCLK_CMT_CK_BUFH_TEST_R"])
                .extra_int_in("BUFHCE_L_CKINT0", &["HCLK_CMT_CLK_0_B0"])
                .extra_int_in("BUFHCE_L_CKINT1", &["HCLK_CMT_CLK_0_B1"])
                .extra_int_in("BUFHCE_R_CKINT0", &["HCLK_CMT_CLK_1_B0"])
                .extra_int_in("BUFHCE_R_CKINT1", &["HCLK_CMT_CLK_1_B1"])
                .extra_wire("MMCM0_CLKIN1_HCLK_L", &["HCLK_CMT_CK_OUT2CMT_L2"])
                .extra_wire("MMCM0_CLKIN1_HCLK_R", &["HCLK_CMT_CK_OUT2CMT_EXT_R2"])
                .extra_wire("MMCM1_CLKIN1_HCLK_L", &["HCLK_CMT_CK_OUT2CMT_EXT_L2"])
                .extra_wire("MMCM1_CLKIN1_HCLK_R", &["HCLK_CMT_CK_OUT2CMT_R2"])
                .extra_wire("MMCM0_CLKIN2_HCLK_L", &["HCLK_CMT_CK_OUT2CMT_L1"])
                .extra_wire("MMCM0_CLKIN2_HCLK_R", &["HCLK_CMT_CK_OUT2CMT_EXT_R1"])
                .extra_wire("MMCM1_CLKIN2_HCLK_L", &["HCLK_CMT_CK_OUT2CMT_EXT_L1"])
                .extra_wire("MMCM1_CLKIN2_HCLK_R", &["HCLK_CMT_CK_OUT2CMT_R1"])
                .extra_wire("MMCM0_CLKFBIN_HCLK_L", &["HCLK_CMT_CK_OUT2CMT_L0"])
                .extra_wire("MMCM0_CLKFBIN_HCLK_R", &["HCLK_CMT_CK_OUT2CMT_EXT_R0"])
                .extra_wire("MMCM1_CLKFBIN_HCLK_L", &["HCLK_CMT_CK_OUT2CMT_EXT_L0"])
                .extra_wire("MMCM1_CLKFBIN_HCLK_R", &["HCLK_CMT_CK_OUT2CMT_R0"]);
            for i in 0..32 {
                bel = bel
                    .extra_wire(format!("GCLK{i}"), &[format!("HCLK_CMT_CK_GCLK{i}")])
                    .extra_wire(
                        format!("GCLK{i}_INV"),
                        &[format!("HCLK_CMT_CK_GCLK_INV_TEST{i}")],
                    )
                    .extra_wire(
                        format!("GCLK{i}_NOINV"),
                        &[format!("HCLK_CMT_CK_GCLK_NOINV_TEST{i}")],
                    )
                    .extra_wire(
                        format!("GCLK{i}_TEST"),
                        &[format!("HCLK_CMT_CK_GCLK_TEST{i}")],
                    )
                    .extra_wire(
                        format!("CASCO{i}"),
                        &[
                            format!("HCLK_CMT_BOT_CK_BUFG_CASCO{i}"),
                            format!("HCLK_CMT_TOP_CK_BUFG_CASCO{i}"),
                        ],
                    )
                    .extra_wire(
                        format!("CASCI{i}"),
                        &[
                            format!("HCLK_CMT_BOT_CK_BUFG_CASCIN{i}"),
                            format!("HCLK_CMT_TOP_CK_BUFG_CASCIN{i}"),
                        ],
                    );
            }
            for i in 0..4 {
                bel = bel
                    .extra_wire(format!("CCIO{i}_L"), &[format!("HCLK_CMT_CK_CCIO_L{i}")])
                    .extra_wire(format!("CCIO{i}_R"), &[format!("HCLK_CMT_CK_CCIO_R{i}")]);
            }
            for i in 0..8 {
                bel = bel.extra_wire(format!("GIO{i}"), &[format!("HCLK_CMT_CK_IO_TO_CMT{i}")]);
            }
            for i in 0..12 {
                bel = bel
                    .extra_wire(
                        format!("HCLK{i}_L_O"),
                        &[format!("HCLK_CMT_CK_BUFH2QBUF_L{i}")],
                    )
                    .extra_wire(
                        format!("HCLK{i}_R_O"),
                        &[format!("HCLK_CMT_CK_BUFH2QBUF_R{i}")],
                    )
                    .extra_wire(format!("HCLK{i}_L_I"), &[format!("HCLK_CMT_CK_HCLK_L{i}")])
                    .extra_wire(format!("HCLK{i}_R_I"), &[format!("HCLK_CMT_CK_HCLK_R{i}")]);
            }
            for i in 0..6 {
                bel = bel
                    .extra_wire(format!("RCLK{i}_L_I"), &[format!("HCLK_CMT_CK_RCLK_L{i}")])
                    .extra_wire(format!("RCLK{i}_R_I"), &[format!("HCLK_CMT_CK_RCLK_R{i}")]);
            }
            for i in 0..10 {
                bel = bel
                    .extra_wire(format!("MGT{i}_L"), &[format!("HCLK_CMT_CK_MGT_L{i}")])
                    .extra_wire(format!("MGT{i}_R"), &[format!("HCLK_CMT_CK_MGT_R{i}")]);
            }
            for (bt, key) in [('B', "MMCM0"), ('T', "MMCM1")] {
                bel = bel
                    .extra_wire(
                        format!("{key}_CLKIN1_HCLK"),
                        &[format!("HCLK_CMT_CLKIN1_HCLK_{bt}")],
                    )
                    .extra_wire(
                        format!("{key}_CLKIN1_IO"),
                        &[format!("HCLK_CMT_CLKIN1_IO_{bt}")],
                    )
                    .extra_wire(
                        format!("{key}_CLKIN1_MGT"),
                        &[format!("HCLK_CMT_CLKIN1_MGT_{bt}")],
                    )
                    .extra_wire(
                        format!("{key}_CLKIN2_HCLK"),
                        &[format!("HCLK_CMT_CLKIN2_HCLK_{bt}")],
                    )
                    .extra_wire(
                        format!("{key}_CLKIN2_IO"),
                        &[format!("HCLK_CMT_CLKIN2_IO_{bt}")],
                    )
                    .extra_wire(
                        format!("{key}_CLKIN2_MGT"),
                        &[format!("HCLK_CMT_CLKIN2_MGT_{bt}")],
                    )
                    .extra_wire(
                        format!("{key}_CLKFBIN_HCLK"),
                        &[format!("HCLK_CMT_CLKFB_HCLK_{bt}")],
                    )
                    .extra_wire(
                        format!("{key}_CLKFBIN_IO"),
                        &[format!("HCLK_CMT_CLKFB_IO_{bt}")],
                    );
            }
            for i in 0..14 {
                bel = bel
                    .extra_wire(
                        format!("MMCM0_OUT{i}"),
                        &[format!("HCLK_CMT_CK_CMT_BOT{i}")],
                    )
                    .extra_wire(
                        format!("MMCM1_OUT{i}"),
                        &[format!("HCLK_CMT_CK_CMT_TOP{i}")],
                    );
            }
            for i in 0..4 {
                bel = bel
                    .extra_wire(
                        format!("PERF{i}_OL_I"),
                        &[format!("HCLK_CMT_CK_PERF_OUTER_L{i}")],
                    )
                    .extra_wire(
                        format!("PERF{i}_IL_I"),
                        &[format!("HCLK_CMT_CK_PERF_INNER_L{i}")],
                    )
                    .extra_wire(
                        format!("PERF{i}_IR_I"),
                        &[format!("HCLK_CMT_CK_PERF_INNER_R{i}")],
                    )
                    .extra_wire(
                        format!("PERF{i}_OR_I"),
                        &[format!("HCLK_CMT_CK_PERF_OUTER_R{i}")],
                    )
                    .extra_wire(
                        format!("PERF{i}_OL_O"),
                        &[format!("HCLK_CMT_CK_PERF_OUTER_L{i}_LEFT")],
                    )
                    .extra_wire(
                        format!("PERF{i}_IL_O"),
                        &[format!("HCLK_CMT_CK_PERF_INNER_L{i}_LEFT")],
                    )
                    .extra_wire(
                        format!("PERF{i}_IR_O"),
                        &[format!("HCLK_CMT_CK_PERF_INNER_R{i}_RIGHT")],
                    )
                    .extra_wire(
                        format!("PERF{i}_OR_O"),
                        &[format!("HCLK_CMT_CK_PERF_OUTER_R{i}_RIGHT")],
                    );
            }
            bels.push(bel);
            let mut xn = builder
                .xnode(tslots::BEL, "CMT", naming, xy_bot)
                .num_tiles(40)
                .raw_tile(xy_top)
                .raw_tile(xy);
            for i in 0..20 {
                xn = xn.ref_int(xy_bot.delta(-2, -11 + i as i32), i).ref_single(
                    xy_bot.delta(-1, -11 + i as i32),
                    i,
                    intf,
                )
            }
            for i in 0..20 {
                xn = xn
                    .ref_int(xy_top.delta(-2, -9 + i as i32), i + 20)
                    .ref_single(xy_top.delta(-1, -9 + i as i32), i + 20, intf)
            }
            xn.bels(bels).extract();
        }
    }

    for tkn in ["CMT_PMVA", "CMT_PMVA_BELOW"] {
        if let Some(&xy) = rd.tiles_by_kind_name(tkn).iter().next() {
            let intf = builder.ndb.get_tile_class_naming("INTF");
            let bel = builder.bel_xy(bels::PMVIOB, "PMVIOB", 0, 0);
            builder
                .xnode(tslots::BEL, "PMVIOB", tkn, xy)
                .num_tiles(2)
                .ref_int(xy.delta(-2, 0), 0)
                .ref_int(xy.delta(-2, 1), 1)
                .ref_single(xy.delta(-1, 0), 0, intf)
                .ref_single(xy.delta(-1, 1), 1, intf)
                .bel(bel)
                .extract();
        }
    }

    for tkn in ["CMT_PMVB_BUF_BELOW", "CMT_PMVB_BUF_ABOVE"] {
        if let Some(&xy) = rd.tiles_by_kind_name(tkn).iter().next() {
            let mut bel = builder.bel_virtual(bels::GCLK_BUF);
            for i in 0..32 {
                bel = bel
                    .extra_wire(format!("GCLK{i}_I"), &[format!("CMT_PMVB_CK_GCLK{i}_IN")])
                    .extra_wire(format!("GCLK{i}_O"), &[format!("CMT_PMVB_CK_GCLK{i}_OUT")]);
            }
            for i in 0..8 {
                bel = bel
                    .extra_wire(
                        format!("GIO{i}_I"),
                        &[format!("CMT_PMVB_CK_IO_TO_CMT{i}_IN")],
                    )
                    .extra_wire(
                        format!("GIO{i}_O"),
                        &[format!("CMT_PMVB_CK_IO_TO_CMT{i}_OUT")],
                    );
            }
            builder
                .xnode(tslots::BEL, "GCLK_BUF", "GCLK_BUF", xy)
                .num_tiles(0)
                .bel(bel)
                .extract();
        }
    }

    for tkn in ["CMT_BUFG_BOT", "CMT_BUFG_TOP"] {
        if let Some(&xy) = rd.tiles_by_kind_name(tkn).iter().next() {
            let intf = builder.ndb.get_tile_class_naming("INTF");
            let mut bels = vec![];
            let is_b = tkn == "CMT_BUFG_BOT";
            let bi = if is_b { 0 } else { 16 };
            let int_xy = xy.delta(-2, if is_b { -1 } else { 0 });
            let cmt_xy = xy.delta(0, if is_b { -9 } else { 11 });
            for i in 0..16 {
                let ii = bi + i;
                bels.push(
                    builder
                        .bel_xy(bels::BUFGCTRL[ii], "BUFGCTRL", 0, i)
                        .pins_name_only(&["I0", "I1", "O"])
                        .extra_int_in(
                            "I0_CKINT",
                            &[[
                                "CMT_BUFG_BORROWED_IMUX38",
                                "CMT_BUFG_BORROWED_IMUX25",
                                "CMT_BUFG_BORROWED_IMUX22",
                                "CMT_BUFG_BORROWED_IMUX9",
                                "CMT_BUFG_BORROWED_IMUX6",
                                "CMT_BUFG_IMUX_B1_0",
                                "CMT_BUFG_IMUX_B25_0",
                                "CMT_BUFG_IMUX_B35_0",
                                "CMT_BUFG_IMUX_B12_0",
                                "CMT_BUFG_IMUX_B38_0",
                                "CMT_BUFG_IMUX_B23_0",
                                "CMT_BUFG_IMUX_B33_1",
                                "CMT_BUFG_IMUX_B10_1",
                                "CMT_BUFG_IMUX_B20_1",
                                "CMT_BUFG_IMUX_B5_1",
                                "CMT_BUFG_IMUX_B31_1",
                                "CMT_BUFG_IMUX_B8_0",
                                "CMT_BUFG_IMUX_B18_0",
                                "CMT_BUFG_IMUX_B42_0",
                                "CMT_BUFG_IMUX_B13_0",
                                "CMT_BUFG_IMUX_B37_0",
                                "CMT_BUFG_IMUX_B16_1",
                                "CMT_BUFG_IMUX_B40_1",
                                "CMT_BUFG_IMUX_B3_1",
                                "CMT_BUFG_IMUX_B27_1",
                                "CMT_BUFG_IMUX_B6_1",
                                "CMT_BUFG_IMUX_B30_1",
                                "CMT_BUFG_BORROWED_IMUX6",
                                "CMT_BUFG_BORROWED_IMUX9",
                                "CMT_BUFG_BORROWED_IMUX22",
                                "CMT_BUFG_BORROWED_IMUX25",
                                "CMT_BUFG_BORROWED_IMUX38",
                            ][ii]],
                        )
                        .extra_int_in(
                            "I1_CKINT",
                            &[[
                                "CMT_BUFG_BORROWED_IMUX39",
                                "CMT_BUFG_BORROWED_IMUX24",
                                "CMT_BUFG_BORROWED_IMUX23",
                                "CMT_BUFG_BORROWED_IMUX8",
                                "CMT_BUFG_BORROWED_IMUX7",
                                "CMT_BUFG_IMUX_B9_0",
                                "CMT_BUFG_IMUX_B17_0",
                                "CMT_BUFG_IMUX_B43_0",
                                "CMT_BUFG_IMUX_B4_0",
                                "CMT_BUFG_IMUX_B7_0",
                                "CMT_BUFG_IMUX_B15_0",
                                "CMT_BUFG_IMUX_B41_1",
                                "CMT_BUFG_IMUX_B2_1",
                                "CMT_BUFG_IMUX_B28_1",
                                "CMT_BUFG_IMUX_B36_1",
                                "CMT_BUFG_IMUX_B39_1",
                                "CMT_BUFG_IMUX_B0_0",
                                "CMT_BUFG_IMUX_B26_0",
                                "CMT_BUFG_IMUX_B34_0",
                                "CMT_BUFG_IMUX_B21_0",
                                "CMT_BUFG_IMUX_B29_0",
                                "CMT_BUFG_IMUX_B24_1",
                                "CMT_BUFG_IMUX_B32_1",
                                "CMT_BUFG_IMUX_B11_1",
                                "CMT_BUFG_IMUX_B19_1",
                                "CMT_BUFG_IMUX_B14_1",
                                "CMT_BUFG_IMUX_B22_1",
                                "CMT_BUFG_BORROWED_IMUX7",
                                "CMT_BUFG_BORROWED_IMUX8",
                                "CMT_BUFG_BORROWED_IMUX23",
                                "CMT_BUFG_BORROWED_IMUX24",
                                "CMT_BUFG_BORROWED_IMUX39",
                            ][ii]],
                        )
                        .extra_wire("GCLK", &[format!("CMT_BUFG_CK_GCLK{ii}")])
                        .extra_wire("FB", &[format!("CMT_BUFG_FBG_OUT{i}")])
                        .extra_wire(
                            "I0_CASCI",
                            &[
                                format!("CMT_BUFG_BOT_CK_MUXED{iii}", iii = i * 2),
                                format!("CMT_BUFG_TOP_CK_MUXED{iii}", iii = i * 2),
                            ],
                        )
                        .extra_wire(
                            "I1_CASCI",
                            &[
                                format!("CMT_BUFG_BOT_CK_MUXED{iii}", iii = i * 2 + 1),
                                format!("CMT_BUFG_TOP_CK_MUXED{iii}", iii = i * 2 + 1),
                            ],
                        )
                        .extra_int_in("I0_FB_TEST", &[format!("CMT_BUFG_CK_FB_TEST0_{i}")])
                        .extra_int_in("I1_FB_TEST", &[format!("CMT_BUFG_CK_FB_TEST1_{i}")]),
                );
            }
            let mut bel = builder.bel_virtual(if is_b { bels::GIO_S } else { bels::GIO_N });
            for i in 0..8 {
                bel = bel.extra_wire(
                    format!("GIO{i}_BUFG"),
                    &[
                        format!("CMT_BUFG_BOT_CK_IO_TO_BUFG{i}"),
                        format!("CMT_BUFG_TOP_CK_IO_TO_BUFG{i}"),
                    ],
                );
            }
            if is_b {
                for i in 0..4 {
                    bel = bel
                        .extra_wire(format!("GIO{i}"), &[format!("CMT_BUFG_BOT_CK_PADIN{i}")])
                        .extra_wire(
                            format!("GIO{i}_CMT"),
                            &[
                                format!("CMT_BUFG_BOT_CK_IO_TO_CMT{i}"),
                                format!("CMT_BUFG_TOP_CK_IO_TO_CMT{i}"),
                            ],
                        );
                }
            } else {
                for i in 4..8 {
                    bel = bel
                        .extra_wire(format!("GIO{i}"), &[format!("CMT_BUFG_TOP_CK_PADIN{i}")])
                        .extra_wire(
                            format!("GIO{i}_CMT"),
                            &[
                                format!("CMT_BUFG_BOT_CK_IO_TO_CMT{i}"),
                                format!("CMT_BUFG_TOP_CK_IO_TO_CMT{i}"),
                            ],
                        );
                }
            }
            bels.push(bel);
            builder
                .xnode(tslots::BEL, tkn, tkn, xy)
                .raw_tile(cmt_xy)
                .num_tiles(3)
                .ref_int(int_xy, 0)
                .ref_int(int_xy.delta(0, 1), 1)
                .ref_int(int_xy.delta(0, 2), 2)
                .ref_single(int_xy.delta(1, 0), 0, intf)
                .ref_single(int_xy.delta(1, 1), 1, intf)
                .ref_single(int_xy.delta(1, 2), 2, intf)
                .bels(bels)
                .extract();
        }
    }

    for (tkn, nn) in [("HCLK_GTX", "GTX"), ("HCLK_GTX_LEFT", "GTX_LEFT")] {
        if let Some(&xy) = rd.tiles_by_kind_name(tkn).iter().next() {
            let is_l = tkn == "HCLK_GTX_LEFT";
            let int_xy = xy.delta(if is_l { 2 } else { -3 }, -20);
            let intf_gt =
                builder
                    .ndb
                    .get_tile_class_naming(if is_l { "INTF.GT_L" } else { "INTF.GTX" });
            let mut bel_hclk_gtx = builder
                .bel_virtual(bels::HCLK_GTX)
                .extra_wire("PERFCLK", &["HCLK_GTX_PERFCLK", "HCLK_GTX_LEFT_PERFCLK"])
                .extra_wire(
                    "MGTREFCLKOUT0",
                    &["HCLK_GTX_MGTREFCLKOUT0", "HCLK_GTX_LEFT_MGTREFCLKOUT0"],
                )
                .extra_wire(
                    "MGTREFCLKOUT1",
                    &["HCLK_GTX_MGTREFCLKOUT1", "HCLK_GTX_LEFT_MGTREFCLKOUT1"],
                )
                .extra_wire(
                    "MGTREFCLKIN0",
                    &["HCLK_GTX_MGTREFCLKIN0", "HCLK_GTX_LEFT_MGTREFCLKIN0"],
                )
                .extra_wire(
                    "MGTREFCLKIN1",
                    &["HCLK_GTX_MGTREFCLKIN1", "HCLK_GTX_LEFT_MGTREFCLKIN1"],
                )
                .extra_wire(
                    "SOUTHREFCLKIN0",
                    &["HCLK_GTX_SOUTHREFCLKIN0", "HCLK_GTX_LEFT_SOUTHREFCLKIN0"],
                )
                .extra_wire(
                    "SOUTHREFCLKIN1",
                    &["HCLK_GTX_SOUTHREFCLKIN1", "HCLK_GTX_LEFT_SOUTHREFCLKIN1"],
                )
                .extra_wire(
                    "NORTHREFCLKIN0",
                    &["HCLK_GTX_NORTHREFCLKIN0", "HCLK_GTX_LEFT_NORTHREFCLKIN0"],
                )
                .extra_wire(
                    "NORTHREFCLKIN1",
                    &["HCLK_GTX_NORTHREFCLKIN1", "HCLK_GTX_LEFT_NORTHREFCLKIN1"],
                )
                .extra_wire(
                    "SOUTHREFCLKOUT0",
                    &["HCLK_GTX_SOUTHREFCLKOUT0", "HCLK_GTX_LEFT_SOUTHREFCLKOUT0"],
                )
                .extra_wire(
                    "SOUTHREFCLKOUT1",
                    &["HCLK_GTX_SOUTHREFCLKOUT1", "HCLK_GTX_LEFT_SOUTHREFCLKOUT1"],
                )
                .extra_wire(
                    "NORTHREFCLKOUT0",
                    &["HCLK_GTX_NORTHREFCLKOUT0", "HCLK_GTX_LEFT_NORTHREFCLKOUT0"],
                )
                .extra_wire(
                    "NORTHREFCLKOUT1",
                    &["HCLK_GTX_NORTHREFCLKOUT1", "HCLK_GTX_LEFT_NORTHREFCLKOUT1"],
                );
            for i in 0..10 {
                bel_hclk_gtx = bel_hclk_gtx.extra_wire(
                    format!("MGT{i}"),
                    &[format!("HCLK_GTX_MGT{i}"), format!("HCLK_GTX_LEFT_MGT{i}")],
                );
            }
            for i in 0..4 {
                bel_hclk_gtx = bel_hclk_gtx.extra_wire(
                    format!("PERF{i}"),
                    &[
                        format!("HCLK_GTX_PERF_OUTER{i}"),
                        format!("HCLK_GTX_LEFT_PERF_OUTER{i}"),
                    ],
                );
            }
            for i in 0..4 {
                bel_hclk_gtx = bel_hclk_gtx
                    .extra_wire(
                        format!("RXRECCLK{i}"),
                        &[
                            format!("HCLK_GTX_RXRECCLK{i}"),
                            format!("HCLK_GTX_LEFT_RXRECCLK{i}"),
                        ],
                    )
                    .extra_wire(
                        format!("TXOUTCLK{i}"),
                        &[
                            format!("HCLK_GTX_TXOUTCLK{i}"),
                            format!("HCLK_GTX_LEFT_TXOUTCLK{i}"),
                        ],
                    );
            }
            let mut bels = vec![];
            for i in 0..4 {
                bels.extend([
                    builder
                        .bel_xy(bels::IPAD_RXP[i], "IPAD", 0, 1)
                        .raw_tile(i + 1)
                        .pins_name_only(&["O"]),
                    builder
                        .bel_xy(bels::IPAD_RXN[i], "IPAD", 0, 0)
                        .raw_tile(i + 1)
                        .pins_name_only(&["O"]),
                    builder
                        .bel_xy(bels::OPAD_TXP[i], "OPAD", 0, 1)
                        .raw_tile(i + 1)
                        .pins_name_only(&["I"]),
                    builder
                        .bel_xy(bels::OPAD_TXN[i], "OPAD", 0, 0)
                        .raw_tile(i + 1)
                        .pins_name_only(&["I"]),
                ]);
            }
            bels.extend([
                builder
                    .bel_xy(bels::IPAD_CLKP0, "IPAD", 0, 2)
                    .pins_name_only(&["O"]),
                builder
                    .bel_xy(bels::IPAD_CLKN0, "IPAD", 0, 3)
                    .pins_name_only(&["O"]),
                builder
                    .bel_xy(bels::IPAD_CLKP1, "IPAD", 0, 0)
                    .pins_name_only(&["O"]),
                builder
                    .bel_xy(bels::IPAD_CLKN1, "IPAD", 0, 1)
                    .pins_name_only(&["O"]),
            ]);
            for i in 0..4 {
                bels.push(
                    builder
                        .bel_xy(bels::GTX[i], "GTXE1", 0, 0)
                        .raw_tile(i + 1)
                        .pins_name_only(&[
                            "RXP",
                            "RXN",
                            "TXP",
                            "TXN",
                            "PERFCLKRX",
                            "PERFCLKTX",
                            "MGTREFCLKRX0",
                            "MGTREFCLKRX1",
                            "MGTREFCLKTX0",
                            "MGTREFCLKTX1",
                            "NORTHREFCLKRX0",
                            "NORTHREFCLKRX1",
                            "NORTHREFCLKTX0",
                            "NORTHREFCLKTX1",
                            "SOUTHREFCLKRX0",
                            "SOUTHREFCLKRX1",
                            "SOUTHREFCLKTX0",
                            "SOUTHREFCLKTX1",
                        ])
                        .pin_name_only("TXOUTCLK", 1)
                        .pin_name_only("RXRECCLK", 1)
                        .extra_wire("PERFCLK", &["GTX_PERFCLK", "GTX_LEFT_PERCLK"])
                        .extra_wire(
                            "MGTREFCLKOUT0",
                            &["GTX_MGTREFCLKOUT0", "GTX_LEFT_MGTREFCLKOUT0"],
                        )
                        .extra_wire(
                            "MGTREFCLKOUT1",
                            &["GTX_MGTREFCLKOUT1", "GTX_LEFT_MGTREFCLKOUT1"],
                        )
                        .extra_wire(
                            "NORTHREFCLKIN0",
                            &["GTX_NORTHREFCLKIN0", "GTX_LEFT_NORTHREFCLKIN0"],
                        )
                        .extra_wire(
                            "NORTHREFCLKIN1",
                            &["GTX_NORTHREFCLKIN1", "GTX_LEFT_NORTHREFCLKIN1"],
                        )
                        .extra_wire(
                            "SOUTHREFCLKOUT0",
                            &["GTX_SOUTHREFCLKOUT0", "GTX_LEFT_SOUTHREFCLKOUT0"],
                        )
                        .extra_wire(
                            "SOUTHREFCLKOUT1",
                            &["GTX_SOUTHREFCLKOUT1", "GTX_LEFT_SOUTHREFCLKOUT1"],
                        ),
                );
            }
            bels.extend([
                builder
                    .bel_xy(bels::BUFDS0, "IBUFDS_GTXE1", 0, 0)
                    .pins_name_only(&["O", "ODIV2", "I", "IB", "CLKTESTSIG"])
                    .extra_wire(
                        "HCLK_OUT",
                        &["HCLK_GTX_REFCLKHROW0", "HCLK_GTX_LEFT_REFCLKHROW0"],
                    )
                    .extra_int_in(
                        "CLKTESTSIG_INT",
                        &[
                            "IBUFDS_GTXE1_0_CLKTESTSIG_SEG",
                            // sigh. that is an O.
                            "IBUFDS_GTXE1_LEFT_O_CLKTESTSIG_SEG",
                        ],
                    ),
                builder
                    .bel_xy(bels::BUFDS1, "IBUFDS_GTXE1", 0, 1)
                    .pins_name_only(&["O", "ODIV2", "I", "IB", "CLKTESTSIG"])
                    .extra_wire(
                        "HCLK_OUT",
                        &["HCLK_GTX_REFCLKHROW1", "HCLK_GTX_LEFT_REFCLKHROW1"],
                    )
                    .extra_int_in(
                        "CLKTESTSIG_INT",
                        &[
                            "IBUFDS_GTXE1_1_CLKTESTSIG_SEG",
                            "IBUFDS_GTXE1_LEFT_1_CLKTESTSIG_SEG",
                        ],
                    ),
                bel_hclk_gtx,
            ]);
            let mut xn = builder
                .xnode(tslots::BEL, "GTX", nn, xy)
                .num_tiles(40)
                .raw_tile(xy.delta(0, -20))
                .raw_tile(xy.delta(0, -10))
                .raw_tile(xy.delta(0, 1))
                .raw_tile(xy.delta(0, 11));
            for i in 0..40 {
                xn = xn
                    .ref_int(int_xy.delta(0, (i + i / 20) as i32), i)
                    .ref_single(int_xy.delta(1, (i + i / 20) as i32), i, intf_gt);
            }
            xn.bels(bels).extract();
        }
    }
    for tkn in ["HCLK_GTH_LEFT", "HCLK_GTH"] {
        if let Some(&xy) = rd.tiles_by_kind_name(tkn).iter().next() {
            let is_l = tkn == "HCLK_GTH_LEFT";
            let int_xy = xy.delta(if is_l { 2 } else { -3 }, 0);
            let intf_gt =
                builder
                    .ndb
                    .get_tile_class_naming(if is_l { "INTF.GT_L" } else { "INTF.GTX" });
            let xy_bot = xy.delta(0, -10);
            let xy_top = xy.delta(0, 11);
            let mut bels = vec![];
            for i in 0..4 {
                bels.extend([
                    builder
                        .bel_xy(bels::IPAD_RXP[i], "IPAD", 0, (3 - i) * 2 + 1)
                        .raw_tile(1)
                        .pins_name_only(&["O"]),
                    builder
                        .bel_xy(bels::IPAD_RXN[i], "IPAD", 0, (3 - i) * 2)
                        .raw_tile(1)
                        .pins_name_only(&["O"]),
                ]);
            }
            for i in 0..4 {
                bels.extend([
                    builder
                        .bel_xy(bels::OPAD_TXP[i], "OPAD", 0, (3 - i) * 2 + 1)
                        .raw_tile(1)
                        .pins_name_only(&["I"]),
                    builder
                        .bel_xy(bels::OPAD_TXN[i], "OPAD", 0, (3 - i) * 2)
                        .raw_tile(1)
                        .pins_name_only(&["I"]),
                ]);
            }
            bels.extend([
                builder
                    .bel_xy(bels::IPAD_CLKP0, "IPAD", 0, 1)
                    .raw_tile(2)
                    .pins_name_only(&["O"]),
                builder
                    .bel_xy(bels::IPAD_CLKN0, "IPAD", 0, 0)
                    .raw_tile(2)
                    .pins_name_only(&["O"]),
            ]);
            let mut bel_gt = builder
                .bel_xy(bels::GTH_QUAD, "GTHE1_QUAD", 0, 0)
                .raw_tile(1)
                .pins_name_only(&["TSTPATH", "TSTREFCLKOUT"])
                .pin_name_only("REFCLK", 1)
                .extra_int_in("GREFCLK", &["GTH_LEFT_GREFCLK", "GTHE1_RIGHT_GREFCLK"])
                .extra_wire(
                    "REFCLK_IN",
                    &["GTH_LEFT_IBUF_OUTCLK", "GTHE1_RIGHT_IBUF_OUTCLK"],
                )
                .extra_wire(
                    "REFCLK_SOUTH",
                    &["GTH_LEFT_REFCLKSOUTHIN", "GTHE1_RIGHT_REFCLKSOUTHIN"],
                )
                .extra_wire(
                    "REFCLK_NORTH",
                    &["GTH_LEFT_REFCLKNORTHIN", "GTHE1_RIGHT_REFCLKNORTHIN"],
                )
                .extra_wire("REFCLK_UP", &["GTH_TOP_REFCLKUP", "GTH_LEFT_REFCLK_NORTH"])
                .extra_wire("REFCLK_DN", &["GTH_TOP_REFCLKDN", "GTH_LEFT_REFCLK_SOUTH"]);
            for i in 0..4 {
                bel_gt = bel_gt.pins_name_only(&[
                    format!("RXP{i}"),
                    format!("RXN{i}"),
                    format!("TXP{i}"),
                    format!("TXN{i}"),
                    format!("TXUSERCLKOUT{i}"),
                    format!("RXUSERCLKOUT{i}"),
                ]);
            }
            for i in 0..10 {
                bel_gt = bel_gt.extra_wire(
                    format!("MGT{i}"),
                    &[
                        format!("GTH_LEFT_MGTCLK{i}"),
                        format!("GTHE1_RIGHT_MGTCLK{i}"),
                    ],
                );
            }
            bels.push(bel_gt);
            bels.push(
                builder
                    .bel_xy(bels::BUFDS0, "IBUFDS_GTHE1", 0, 0)
                    .raw_tile(2)
                    .pins_name_only(&["I", "IB"])
                    .pin_name_only("O", 1),
            );
            let mut bel = builder.bel_virtual(bels::HCLK_GTH).raw_tile(2);
            for i in 0..10 {
                bel = bel
                    .extra_wire(
                        format!("MGT{i}"),
                        &[
                            format!("HCLK_GTH_LEFT_MGT{i}"),
                            format!("HCLK_GTH_RIGHT_MGT{i}"),
                        ],
                    )
                    .extra_wire(
                        format!("MGT{i}_I"),
                        &[
                            format!("HCLK_GTH_LEFT_MGTCLK{i}"),
                            format!("HCLK_GTH_RIGHT_MGTCLK{i}"),
                        ],
                    );
            }
            bels.push(bel);

            let mut xn = builder
                .xnode(
                    tslots::BEL,
                    "GTH",
                    if is_l { "GTH.L" } else { "GTH.R" },
                    xy_bot,
                )
                .num_tiles(40)
                .raw_tile(xy_top)
                .raw_tile(xy);
            for i in 0..20 {
                xn = xn.ref_int(int_xy.delta(0, -20 + i as i32), i).ref_single(
                    int_xy.delta(1, -20 + i as i32),
                    i,
                    intf_gt,
                )
            }
            for i in 0..20 {
                xn = xn
                    .ref_int(int_xy.delta(0, 1 + i as i32), i + 20)
                    .ref_single(int_xy.delta(1, 1 + i as i32), i + 20, intf_gt)
            }
            xn.bels(bels).extract();
        }
    }

    for (tkn, naming) in [
        ("HCLK_CLBLM_MGT_LEFT", "MGT_BUF.L"),
        ("HCLK_CLBLM_MGT", "MGT_BUF.R"),
    ] {
        if let Some(&xy) = rd.tiles_by_kind_name(tkn).iter().next() {
            let mut bel = builder.bel_virtual(bels::MGT_BUF);
            for i in 0..10 {
                if naming == "MGT_BUF.L" {
                    bel = bel
                        .extra_wire(format!("MGT{i}_O"), &[format!("HCLK_CLB_MGT_CK_IN_MGT{i}")])
                        .extra_wire(
                            format!("MGT{i}_I"),
                            &[format!("HCLK_CLB_MGT_CK_OUT_MGT{i}")],
                        );
                } else {
                    bel = bel
                        .extra_wire(
                            format!("MGT{i}_O"),
                            &[format!("HCLK_CLB_MGT_CK_OUT_MGT{i}")],
                        )
                        .extra_wire(format!("MGT{i}_I"), &[format!("HCLK_CLB_MGT_CK_IN_MGT{i}")]);
                }
            }
            builder
                .xnode(tslots::CLK, "MGT_BUF", naming, xy)
                .num_tiles(0)
                .bel(bel)
                .extract();
        }
    }

    builder.build()
}
