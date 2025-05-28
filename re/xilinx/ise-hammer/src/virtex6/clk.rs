use prjcombine_interconnect::grid::NodeLoc;
use prjcombine_re_fpga_hammer::{Diff, OcdMode, xlat_bit, xlat_enum_ocd};
use prjcombine_re_hammer::Session;
use prjcombine_re_xilinx_geom::ExpandedDevice;
use prjcombine_virtex4::bels;

use crate::{
    backend::IseBackend,
    collector::CollectorCtx,
    generic::{
        fbuild::{FuzzBuilderBase, FuzzCtx},
        props::relation::{Delta, NodeRelation},
    },
};

#[derive(Clone, Debug)]
struct Cmt;

impl NodeRelation for Cmt {
    fn resolve(&self, backend: &IseBackend, nloc: NodeLoc) -> Option<NodeLoc> {
        let ExpandedDevice::Virtex4(edev) = backend.edev else {
            unreachable!()
        };
        Some(
            edev.egrid
                .get_tile_by_bel((nloc.0, (edev.col_clk, nloc.2), bels::CMT)),
        )
    }
}

pub fn add_fuzzers<'a>(session: &mut Session<'a, IseBackend<'a>>, backend: &'a IseBackend<'a>) {
    {
        let mut ctx = FuzzCtx::new(session, backend, "HCLK");
        let mut bctx = ctx.bel(bels::HCLK);
        for i in 0..8 {
            for ud in ['U', 'D'] {
                for j in 0..12 {
                    bctx.build()
                        .global_mutex("HCLK", "USE")
                        .row_mutex("BUFH_TEST", format!("USED_HCLK{j}"))
                        .related_pip(
                            Cmt,
                            (bels::CMT, "BUFH_TEST_L_PRE"),
                            (bels::CMT, format!("HCLK{j}_L_I")),
                        )
                        .related_pip(
                            Cmt,
                            (bels::CMT, "BUFH_TEST_R_PRE"),
                            (bels::CMT, format!("HCLK{j}_R_I")),
                        )
                        .mutex(format!("MUX.LCLK{i}_{ud}"), format!("HCLK{j}"))
                        .mutex(format!("HCLK{j}"), format!("MUX.LCLK{i}_{ud}"))
                        .test_manual(format!("MUX.LCLK{i}_{ud}"), format!("HCLK{j}"))
                        .pip(format!("LCLK{i}_{ud}"), format!("HCLK{j}"))
                        .commit();
                }
                for j in 0..6 {
                    bctx.build()
                        .global_mutex("RCLK", "USE")
                        .row_mutex("BUFH_TEST", format!("USED_RCLK{j}"))
                        .related_pip(
                            Cmt,
                            (bels::CMT, "BUFH_TEST_L_PRE"),
                            (bels::CMT, format!("RCLK{j}_L_I")),
                        )
                        .related_pip(
                            Cmt,
                            (bels::CMT, "BUFH_TEST_R_PRE"),
                            (bels::CMT, format!("RCLK{j}_R_I")),
                        )
                        .mutex(format!("MUX.LCLK{i}_{ud}"), format!("RCLK{j}"))
                        .mutex(format!("RCLK{j}"), format!("MUX.LCLK{i}_{ud}"))
                        .test_manual(format!("MUX.LCLK{i}_{ud}"), format!("RCLK{j}"))
                        .pip(format!("LCLK{i}_{ud}"), format!("RCLK{j}"))
                        .commit();
                }
            }
        }
    }

    for (tile, gio, base) in [
        ("CMT_BUFG_BOT", bels::GIO_S, 0),
        ("CMT_BUFG_TOP", bels::GIO_N, 16),
    ] {
        let mut ctx = FuzzCtx::new(session, backend, tile);
        for i in 0..16 {
            let mut bctx = ctx.bel(bels::BUFGCTRL[base + i]);
            let mode = "BUFGCTRL";
            bctx.build().test_manual("PRESENT", "1").mode(mode).commit();
            for pin in ["CE0", "CE1", "S0", "S1", "IGNORE0", "IGNORE1"] {
                bctx.mode(mode).test_inv(pin);
            }
            bctx.mode(mode)
                .test_enum("PRESELECT_I0", &["FALSE", "TRUE"]);
            bctx.mode(mode)
                .test_enum("PRESELECT_I1", &["FALSE", "TRUE"]);
            bctx.mode(mode).test_enum("CREATE_EDGE", &["FALSE", "TRUE"]);
            bctx.mode(mode).test_enum("INIT_OUT", &["0", "1"]);
            bctx.build()
                .test_manual("ENABLE.FB", "1")
                .pip("FB", "O")
                .commit();
            bctx.build()
                .null_bits()
                .extra_tile(Delta::new(0, -20, "CMT"), "CMT")
                .extra_tile(Delta::new(0, 20, "CMT"), "CMT")
                .global_mutex("GCLK", "TEST")
                .test_manual(format!("ENABLE.GCLK{}", base + i), "1")
                .pip("GCLK", "O")
                .commit();
            // ISE bug causes pips to be reversed?
            bctx.build()
                .mutex("MUX.I0", "FB_TEST")
                .test_manual("TEST_I0", "1")
                .pip("I0", "I0_FB_TEST")
                .commit();
            bctx.build()
                .mutex("MUX.I1", "FB_TEST")
                .test_manual("TEST_I1", "1")
                .pip("I1", "I1_FB_TEST")
                .commit();
            for j in 0..2 {
                for k in 0..8 {
                    bctx.build()
                        .mutex(format!("MUX.I{j}"), format!("GIO{k}"))
                        .test_manual(format!("MUX.I{j}"), format!("GIO{k}"))
                        .pip(format!("I{j}"), (gio, format!("GIO{k}_BUFG")))
                        .commit();
                }
                bctx.build()
                    .mutex(format!("MUX.I{j}"), "CASCI")
                    .test_manual(format!("MUX.I{j}"), "CASCI")
                    .pip(format!("I{j}"), format!("I{j}_CASCI"))
                    .commit();
                bctx.build()
                    .mutex(format!("MUX.I{j}"), "CKINT")
                    .test_manual(format!("MUX.I{j}"), "CKINT")
                    .pip(format!("I{j}"), format!("I{j}_CKINT"))
                    .commit();
                let obel_prev = bels::BUFGCTRL[base + (i + 15) % 16];
                bctx.build()
                    .mutex(format!("MUX.I{j}"), "FB_PREV")
                    .test_manual(format!("MUX.I{j}"), "FB_PREV")
                    .pip(format!("I{j}"), (obel_prev, "FB"))
                    .commit();
                let obel_next = bels::BUFGCTRL[base + (i + 1) % 16];
                bctx.build()
                    .mutex(format!("MUX.I{j}"), "FB_NEXT")
                    .test_manual(format!("MUX.I{j}"), "FB_NEXT")
                    .pip(format!("I{j}"), (obel_next, "FB"))
                    .commit();
            }
        }
        let mut bctx = ctx.bel(gio);
        let gio_base = base / 4;
        for i in gio_base..(gio_base + 4) {
            bctx.build()
                .null_bits()
                .global_mutex("GIO", "TEST")
                .extra_tile(Delta::new(0, -20, "CMT"), "CMT")
                .extra_tile(Delta::new(0, 20, "CMT"), "CMT")
                .test_manual(format!("ENABLE.GIO{i}"), "1")
                .pip(format!("GIO{i}_CMT"), format!("GIO{i}"))
                .commit();
        }
    }
    {
        let mut ctx = FuzzCtx::new(session, backend, "HCLK_IOI");
        for i in 0..4 {
            let mut bctx = ctx.bel(bels::BUFIO[i]);
            bctx.test_manual("PRESENT", "1").mode("BUFIODQS").commit();
            bctx.mode("BUFIODQS")
                .test_enum("DQSMASK_ENABLE", &["FALSE", "TRUE"]);
            bctx.build()
                .mutex("MUX.I", "PERF")
                .test_manual("MUX.I", format!("PERF{}", i ^ 1))
                .pip(
                    (bels::HCLK_IOI, format!("IOCLK_IN{i}")),
                    (bels::HCLK_IOI, format!("PERF{}_BUF", i ^ 1)),
                )
                .commit();
            bctx.build()
                .mutex("MUX.I", "CCIO")
                .test_manual("MUX.I", "CCIO")
                .pip(
                    (bels::HCLK_IOI, format!("IOCLK_IN{i}")),
                    (bels::HCLK_IOI, format!("IOCLK_PAD{i}")),
                )
                .commit();
            bctx.test_manual("ENABLE", "1")
                .pip((bels::HCLK_IOI, format!("IOCLK{i}_PRE")), "O")
                .commit();
        }
        for i in 0..2 {
            let mut bctx = ctx.bel(bels::BUFR[i]);
            let bel_other = bels::BUFR[i ^ 1];
            bctx.build()
                .global_mutex("RCLK", "BUFR")
                .test_manual("ENABLE", "1")
                .mode("BUFR")
                .commit();
            bctx.mode("BUFR").global_mutex("RCLK", "BUFR").test_enum(
                "BUFR_DIVIDE",
                &["BYPASS", "1", "2", "3", "4", "5", "6", "7", "8"],
            );
            for j in 0..10 {
                bctx.build()
                    .row_mutex("MGT", "USE")
                    .mutex("MUX.I", format!("MGT{j}"))
                    .bel_mutex(bel_other, "MUX.I", format!("MGT{j}"))
                    .pip((bel_other, "I"), (bels::HCLK_IOI, format!("MGT{j}")))
                    .test_manual("MUX.I", format!("MGT{j}"))
                    .pip("I", (bels::HCLK_IOI, format!("MGT{j}")))
                    .commit();
            }
            for j in 0..4 {
                bctx.build()
                    .mutex("MUX.I", format!("BUFIO{j}_I"))
                    .test_manual("MUX.I", format!("BUFIO{j}_I"))
                    .pip("I", (bels::HCLK_IOI, format!("IOCLK_IN{j}_BUFR")))
                    .commit();
            }
            for j in 0..2 {
                bctx.build()
                    .mutex("MUX.I", format!("CKINT{j}"))
                    .test_manual("MUX.I", format!("CKINT{j}"))
                    .pip("I", (bels::HCLK_IOI, format!("BUFR_CKINT{j}")))
                    .commit();
            }
        }
        for i in 0..2 {
            let mut bctx = ctx.bel(bels::BUFO[i]);
            bctx.test_manual("PRESENT", "1").mode("BUFO").commit();
            for (val, pin) in [
                (format!("VOCLK{i}"), "I_PRE"),
                (format!("VOCLK{i}_S"), "VI_S"),
                (format!("VOCLK{i}_N"), "VI_N"),
            ] {
                bctx.build()
                    .mutex("MUX.I", pin)
                    .has_related(Delta::new(0, 40, "HCLK_IOI"))
                    .has_related(Delta::new(0, -40, "HCLK_IOI"))
                    .test_manual("MUX.I", val)
                    .pip("I", pin)
                    .commit();
            }
        }
        {
            let mut bctx = ctx.bel(bels::IDELAYCTRL);
            for i in 0..12 {
                bctx.build()
                    .mutex("MUX.REFCLK", format!("HCLK{i}"))
                    .test_manual("MUX.REFCLK", format!("HCLK{i}"))
                    .pip("REFCLK", (bels::HCLK_IOI, format!("HCLK{i}_O")))
                    .commit();
            }
            bctx.test_manual("PRESENT", "1").mode("IDELAYCTRL").commit();
            bctx.mode("IDELAYCTRL")
                .test_enum("RESET_STYLE", &["V4", "V5"]);
            bctx.mode("IDELAYCTRL")
                .test_enum("HIGH_PERFORMANCE_MODE", &["FALSE", "TRUE"]);
            bctx.mode("IDELAYCTRL")
                .tile_mutex("IDELAYCTRL", "TEST")
                .test_manual("MODE", "DEFAULT")
                .attr("IDELAYCTRL_EN", "DEFAULT")
                .attr("BIAS_MODE", "2")
                .commit();
            bctx.mode("IDELAYCTRL")
                .tile_mutex("IDELAYCTRL", "TEST")
                .test_manual("MODE", "FULL_0")
                .attr("IDELAYCTRL_EN", "ENABLE")
                .attr("BIAS_MODE", "0")
                .commit();
            bctx.mode("IDELAYCTRL")
                .tile_mutex("IDELAYCTRL", "TEST")
                .test_manual("MODE", "FULL_1")
                .attr("IDELAYCTRL_EN", "ENABLE")
                .attr("BIAS_MODE", "1")
                .commit();
        }
        {
            let mut bctx = ctx.bel(bels::HCLK_IOI);
            for i in 0..12 {
                bctx.build()
                    .global_mutex("HCLK", "USE")
                    .row_mutex("BUFH_TEST", format!("USED_HCLK{i}"))
                    .related_pip(
                        Cmt,
                        (bels::CMT, "BUFH_TEST_L_PRE"),
                        (bels::CMT, format!("HCLK{i}_L_I")),
                    )
                    .related_pip(
                        Cmt,
                        (bels::CMT, "BUFH_TEST_R_PRE"),
                        (bels::CMT, format!("HCLK{i}_R_I")),
                    )
                    .test_manual(format!("BUF.HCLK{i}"), "1")
                    .pip(format!("HCLK{i}_O"), format!("HCLK{i}_I"))
                    .commit();
            }
            for i in 0..6 {
                bctx.build()
                    .global_mutex("RCLK", "USE")
                    .row_mutex("BUFH_TEST", format!("USED_RCLK{i}"))
                    .related_pip(
                        Cmt,
                        (bels::CMT, "BUFH_TEST_L_PRE"),
                        (bels::CMT, format!("RCLK{i}_L_I")),
                    )
                    .related_pip(
                        Cmt,
                        (bels::CMT, "BUFH_TEST_R_PRE"),
                        (bels::CMT, format!("RCLK{i}_R_I")),
                    )
                    .test_manual(format!("BUF.RCLK{i}"), "1")
                    .pip(format!("RCLK{i}_O"), format!("RCLK{i}_I"))
                    .commit();
                for pin in [
                    "VRCLK0", "VRCLK1", "VRCLK0_S", "VRCLK1_S", "VRCLK0_N", "VRCLK1_N",
                ] {
                    bctx.build()
                        .global_mutex("RCLK", "USE")
                        .row_mutex_here("RCLK_DRIVE")
                        .mutex(format!("MUX.RCLK{i}"), pin)
                        .row_mutex("BUFH_TEST", format!("USED_RCLK{i}"))
                        .related_pip(
                            Cmt,
                            (bels::CMT, "BUFH_TEST_L_PRE"),
                            (bels::CMT, format!("RCLK{i}_L_I")),
                        )
                        .related_pip(
                            Cmt,
                            (bels::CMT, "BUFH_TEST_R_PRE"),
                            (bels::CMT, format!("RCLK{i}_R_I")),
                        )
                        .test_manual(format!("MUX.RCLK{i}"), pin)
                        .pip(format!("RCLK{i}_I"), pin)
                        .commit();
                }
            }
            for i in 0..4 {
                bctx.test_manual(format!("BUF.PERF{i}"), "1")
                    .pip(format!("PERF{i}_BUF"), format!("PERF{i}"))
                    .commit();
            }
            for (i, pre) in [
                "IOCLK0_PRE",
                "IOCLK1_PRE",
                "IOCLK2_PRE",
                "IOCLK3_PRE",
                "IOCLK0_PRE_S",
                "IOCLK3_PRE_S",
                "IOCLK0_PRE_N",
                "IOCLK3_PRE_N",
            ]
            .into_iter()
            .enumerate()
            {
                bctx.build()
                    .mutex(format!("DELAY.IOCLK{i}"), "0")
                    .test_manual(format!("DELAY.IOCLK{i}"), "0")
                    .pip(format!("IOCLK{i}"), pre)
                    .commit();
                bctx.build()
                    .mutex(format!("DELAY.IOCLK{i}"), "1")
                    .test_manual(format!("DELAY.IOCLK{i}"), "1")
                    .pip(format!("IOCLK{i}"), format!("IOCLK{i}_DLY"))
                    .pip(format!("IOCLK{i}_DLY"), pre)
                    .commit();
            }
            for i in 0..2 {
                bctx.test_manual(format!("BUF.VOCLK{i}"), "1")
                    .pip((bels::BUFO[i], "I_PRE"), (bels::BUFO[i], "I_PRE2"))
                    .commit();
            }
        }
    }

    {
        let mut ctx = FuzzCtx::new(session, backend, "PMVIOB");
        let mut bctx = ctx.bel(bels::PMVIOB);
        bctx.test_manual("PRESENT", "1").mode("PMVIOB").commit();
        bctx.mode("PMVIOB")
            .test_enum("HSLEW4_IN", &["FALSE", "TRUE"]);
        bctx.mode("PMVIOB")
            .test_enum("PSLEW4_IN", &["FALSE", "TRUE"]);
        bctx.mode("PMVIOB").test_enum("HYS_IN", &["FALSE", "TRUE"]);
    }
}

pub fn collect_fuzzers(ctx: &mut CollectorCtx) {
    {
        let tile = "HCLK";
        let bel = "HCLK";
        for i in 0..12 {
            let (_, _, diff) = Diff::split(
                ctx.state
                    .peek_diff(tile, bel, "MUX.LCLK0_D", format!("HCLK{i}"))
                    .clone(),
                ctx.state
                    .peek_diff(tile, bel, "MUX.LCLK0_U", format!("HCLK{i}"))
                    .clone(),
            );
            ctx.tiledb
                .insert(tile, bel, format!("ENABLE.HCLK{i}"), xlat_bit(diff));
        }
        for i in 0..6 {
            let (_, _, diff) = Diff::split(
                ctx.state
                    .peek_diff(tile, bel, "MUX.LCLK0_D", format!("RCLK{i}"))
                    .clone(),
                ctx.state
                    .peek_diff(tile, bel, "MUX.LCLK0_U", format!("RCLK{i}"))
                    .clone(),
            );
            ctx.tiledb
                .insert(tile, bel, format!("ENABLE.RCLK{i}"), xlat_bit(diff));
        }
        for i in 0..8 {
            for ud in ['U', 'D'] {
                let mux = &format!("MUX.LCLK{i}_{ud}");
                let mut diffs = vec![("NONE".to_string(), Diff::default())];
                for i in 0..12 {
                    let val = format!("HCLK{i}");
                    let mut diff = ctx.state.get_diff(tile, bel, mux, &val);
                    diff.apply_bit_diff(
                        ctx.tiledb.item(tile, bel, &format!("ENABLE.HCLK{i}")),
                        true,
                        false,
                    );
                    diffs.push((val, diff));
                }
                for i in 0..6 {
                    let val = format!("RCLK{i}");
                    let mut diff = ctx.state.get_diff(tile, bel, mux, &val);
                    diff.apply_bit_diff(
                        ctx.tiledb.item(tile, bel, &format!("ENABLE.RCLK{i}")),
                        true,
                        false,
                    );
                    diffs.push((val, diff));
                }
                ctx.tiledb
                    .insert(tile, bel, mux, xlat_enum_ocd(diffs, OcdMode::Mux));
            }
        }
    }
    for (tile, base) in [("CMT_BUFG_BOT", 0), ("CMT_BUFG_TOP", 16)] {
        for i in 0..16 {
            let bel = &format!("BUFGCTRL{}", base + i);
            for pin in ["CE0", "CE1", "S0", "S1", "IGNORE0", "IGNORE1"] {
                ctx.collect_inv(tile, bel, pin);
            }
            for attr in ["PRESELECT_I0", "PRESELECT_I1", "CREATE_EDGE"] {
                ctx.collect_enum_bool(tile, bel, attr, "FALSE", "TRUE");
            }
            ctx.collect_enum_bool(tile, bel, "INIT_OUT", "0", "1");

            // sigh. fucking. ise.
            let mut item = xlat_bit(ctx.state.peek_diff(tile, bel, "MUX.I0", "CASCI").clone());
            assert_eq!(item.bits.len(), 1);
            item.bits[0].bit += 1;
            ctx.tiledb.insert(tile, bel, "TEST_I1", item);
            let mut item = xlat_bit(ctx.state.peek_diff(tile, bel, "MUX.I1", "CASCI").clone());
            assert_eq!(item.bits.len(), 1);
            item.bits[0].bit += 1;
            ctx.tiledb.insert(tile, bel, "TEST_I0", item);

            for attr in ["MUX.I0", "MUX.I1"] {
                ctx.collect_enum_default_ocd(
                    tile,
                    bel,
                    attr,
                    &[
                        "CASCI", "GIO0", "GIO1", "GIO2", "GIO3", "GIO4", "GIO5", "GIO6", "GIO7",
                        "FB_PREV", "FB_NEXT", "CKINT",
                    ],
                    "NONE",
                    OcdMode::Mux,
                );
            }
            ctx.collect_bit(tile, bel, "ENABLE.FB", "1");
            ctx.state.get_diff(tile, bel, "PRESENT", "1").assert_empty();
            ctx.state.get_diff(tile, bel, "TEST_I0", "1").assert_empty();
            ctx.state.get_diff(tile, bel, "TEST_I1", "1").assert_empty();
        }
    }
    {
        let tile = "PMVIOB";
        let bel = "PMVIOB";
        ctx.state.get_diff(tile, bel, "PRESENT", "1").assert_empty();
        ctx.collect_enum_bool(tile, bel, "HYS_IN", "FALSE", "TRUE");
        ctx.collect_enum_bool(tile, bel, "HSLEW4_IN", "FALSE", "TRUE");
        ctx.collect_enum_bool(tile, bel, "PSLEW4_IN", "FALSE", "TRUE");
    }
    for i in 0..4 {
        let tile = "HCLK_IOI";
        let bel = &format!("BUFIO{i}");
        ctx.state.get_diff(tile, bel, "PRESENT", "1").assert_empty();
        ctx.collect_enum_bool(tile, bel, "DQSMASK_ENABLE", "FALSE", "TRUE");
        ctx.collect_enum(
            tile,
            bel,
            "MUX.I",
            &["CCIO".to_string(), format!("PERF{}", i ^ 1)],
        );
        ctx.collect_bit(tile, bel, "ENABLE", "1");
    }
    for i in 0..2 {
        let tile = "HCLK_IOI";
        let bel = &format!("BUFR{i}");
        ctx.collect_bit(tile, bel, "ENABLE", "1");
        ctx.collect_enum(
            tile,
            bel,
            "BUFR_DIVIDE",
            &["BYPASS", "1", "2", "3", "4", "5", "6", "7", "8"],
        );
        ctx.collect_enum_default(
            tile,
            bel,
            "MUX.I",
            &[
                "BUFIO0_I", "BUFIO1_I", "BUFIO2_I", "BUFIO3_I", "MGT0", "MGT1", "MGT2", "MGT3",
                "MGT4", "MGT5", "MGT6", "MGT7", "MGT8", "MGT9", "CKINT0", "CKINT1",
            ],
            "NONE",
        );
    }
    for i in 0..2 {
        let tile = "HCLK_IOI";
        let bel = &format!("BUFO{i}");
        ctx.state.get_diff(tile, bel, "PRESENT", "1").assert_empty();
        ctx.collect_enum(
            tile,
            bel,
            "MUX.I",
            &[
                format!("VOCLK{i}"),
                format!("VOCLK{i}_S"),
                format!("VOCLK{i}_N"),
            ],
        )
    }
    {
        let tile = "HCLK_IOI";
        let bel = "IDELAYCTRL";
        let vals: [_; 12] = core::array::from_fn(|i| format!("HCLK{i}"));
        ctx.collect_enum(tile, bel, "MUX.REFCLK", &vals);
        ctx.state.get_diff(tile, bel, "PRESENT", "1").assert_empty();
        ctx.collect_enum_bool(tile, bel, "HIGH_PERFORMANCE_MODE", "FALSE", "TRUE");
        ctx.collect_enum_default(tile, bel, "MODE", &["DEFAULT", "FULL_0", "FULL_1"], "NONE");
        ctx.collect_enum(tile, bel, "RESET_STYLE", &["V4", "V5"]);
    }
    {
        let tile = "HCLK_IOI";
        let bel = "HCLK_IOI";
        for i in 0..12 {
            ctx.collect_bit(tile, bel, &format!("BUF.HCLK{i}"), "1");
        }
        for i in 0..6 {
            ctx.collect_bit(tile, bel, &format!("BUF.RCLK{i}"), "1");
            ctx.collect_enum_default_ocd(
                tile,
                bel,
                &format!("MUX.RCLK{i}"),
                &[
                    "VRCLK0", "VRCLK1", "VRCLK0_S", "VRCLK1_S", "VRCLK0_N", "VRCLK1_N",
                ],
                "NONE",
                OcdMode::Mux,
            );
        }
        for i in 0..4 {
            ctx.collect_bit(tile, bel, &format!("BUF.PERF{i}"), "1");
        }
        for i in 0..2 {
            ctx.collect_bit(tile, bel, &format!("BUF.VOCLK{i}"), "1");
        }
        for i in 0..4 {
            ctx.collect_enum_bool(tile, bel, &format!("DELAY.IOCLK{i}"), "0", "1");
        }
        for i in 4..8 {
            let diff_buf = ctx
                .state
                .get_diff(tile, bel, format!("DELAY.IOCLK{i}"), "0");
            let diff_delay = ctx
                .state
                .get_diff(tile, bel, format!("DELAY.IOCLK{i}"), "1")
                .combine(&!&diff_buf);
            ctx.tiledb
                .insert(tile, bel, format!("BUF.IOCLK{i}"), xlat_bit(diff_buf));
            ctx.tiledb
                .insert(tile, bel, format!("DELAY.IOCLK{i}"), xlat_bit(diff_delay));
        }
    }
    {
        let tile = "CMT";
        let bel = "CMT";
        for i in 0..32 {
            ctx.collect_bit(tile, bel, &format!("ENABLE.GCLK{i}"), "1");
        }
        for i in 0..8 {
            ctx.collect_bit(tile, bel, &format!("ENABLE.GIO{i}"), "1");
        }
    }
}
