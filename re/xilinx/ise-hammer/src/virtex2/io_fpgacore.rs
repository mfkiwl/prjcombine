use prjcombine_interconnect::{dir::Dir, grid::TileCoord};
use prjcombine_re_fpga_hammer::{
    Diff, FeatureId, FuzzerFeature, FuzzerProp, xlat_bit, xlat_bit_wide, xlat_bool,
};
use prjcombine_re_hammer::{Fuzzer, Session};
use prjcombine_re_xilinx_geom::ExpandedDevice;
use prjcombine_types::bsdata::{TileBit, TileItem};
use prjcombine_virtex2::{bels, tslots};

use crate::{
    backend::IseBackend,
    collector::CollectorCtx,
    generic::{
        fbuild::{FuzzBuilderBase, FuzzCtx},
        props::DynProp,
    },
};

#[derive(Clone, Debug)]
struct IobExtra {
    edge: Dir,
}

impl IobExtra {
    pub fn new(edge: Dir) -> Self {
        Self { edge }
    }
}

impl<'b> FuzzerProp<'b, IseBackend<'b>> for IobExtra {
    fn dyn_clone(&self) -> Box<DynProp<'b>> {
        Box::new(Clone::clone(self))
    }

    fn apply<'a>(
        &self,
        backend: &IseBackend<'a>,
        tcrd: TileCoord,
        mut fuzzer: Fuzzer<IseBackend<'a>>,
    ) -> Option<(Fuzzer<IseBackend<'a>>, bool)> {
        let tcrd = tcrd.tile(tslots::IOB);
        let ExpandedDevice::Virtex2(edev) = backend.edev else {
            unreachable!()
        };
        let edge_match = match self.edge {
            Dir::W => tcrd.col == edev.chip.col_w(),
            Dir::E => tcrd.col == edev.chip.col_e(),
            Dir::S => tcrd.row == edev.chip.row_s(),
            Dir::N => tcrd.row == edev.chip.row_n(),
        };
        if edge_match {
            let tile = &backend.edev[tcrd];
            let fuzzer_id = fuzzer.info.features[0].id.clone();
            fuzzer.info.features.push(FuzzerFeature {
                id: FeatureId {
                    tile: backend.edev.db.tile_classes.key(tile.class).clone(),
                    ..fuzzer_id
                },
                tiles: backend.edev.tile_bits(tcrd),
            });
            Some((fuzzer, false))
        } else {
            Some((fuzzer, true))
        }
    }
}

pub fn add_fuzzers<'a>(session: &mut Session<'a, IseBackend<'a>>, backend: &'a IseBackend<'a>) {
    let tile = "IOI.FC";
    let mut ctx = FuzzCtx::new(session, backend, tile);
    for i in 0..4 {
        let mut bctx = ctx.bel(bels::IBUF[i]);
        let mode = "IBUF";
        bctx.test_manual("ENABLE", "1")
            .mode(mode)
            .prop(IobExtra::new(Dir::W))
            .prop(IobExtra::new(Dir::E))
            .prop(IobExtra::new(Dir::S))
            .prop(IobExtra::new(Dir::N))
            .commit();
        bctx.mode(mode)
            .prop(IobExtra::new(Dir::W))
            .prop(IobExtra::new(Dir::E))
            .prop(IobExtra::new(Dir::S))
            .prop(IobExtra::new(Dir::N))
            .test_manual("ENABLE_O2IPADPATH", "1")
            .attr("ENABLE_O2IPADPATH", "ENABLE_O2IPADPATH")
            .commit();
        bctx.mode(mode)
            .attr("ENABLE_O2IQPATH", "")
            .test_manual("ENABLE_O2IPATH", "1")
            .attr("ENABLE_O2IPATH", "ENABLE_O2IPATH")
            .commit();
        bctx.mode(mode)
            .attr("ENABLE_O2IPATH", "")
            .test_manual("ENABLE_O2IQPATH", "1")
            .attr("ENABLE_O2IQPATH", "ENABLE_O2IQPATH")
            .commit();
        bctx.mode(mode)
            .attr("IFFDMUX", "1")
            .attr("IFF", "#FF")
            .pin("I")
            .pin("IQ")
            .test_enum("IMUX", &["0", "1"]);
        bctx.mode(mode)
            .attr("IMUX", "1")
            .attr("IFF", "#FF")
            .pin("I")
            .pin("IQ")
            .test_enum("IFFDMUX", &["0", "1"]);
        bctx.mode(mode)
            .attr("IFFDMUX", "1")
            .attr("IFF_INIT_ATTR", "INIT1")
            .attr("CEINV", "CE_B")
            .pin("IQ")
            .pin("CE")
            .test_enum("IFF", &["#FF", "#LATCH"]);
        bctx.mode(mode)
            .attr("IFF", "#FF")
            .attr("IFFDMUX", "1")
            .pin("IQ")
            .test_enum("IFFATTRBOX", &["SYNC", "ASYNC"]);
        bctx.mode(mode)
            .attr("IFF", "#FF")
            .attr("IFFDMUX", "1")
            .attr("IFF_SR_ATTR", "SRLOW")
            .pin("IQ")
            .test_enum("IFF_INIT_ATTR", &["INIT0", "INIT1"]);
        bctx.mode(mode)
            .attr("IFF", "#FF")
            .attr("IFFDMUX", "1")
            .attr("IFF_INIT_ATTR", "INIT0")
            .pin("IQ")
            .test_enum("IFF_SR_ATTR", &["SRLOW", "SRHIGH"]);

        for pin in ["CLK", "CE", "SR", "REV"] {
            bctx.mode(mode).pin("IQ").attr("IFF", "#FF").test_inv(pin);
        }
    }
    for i in 0..4 {
        let mut bctx = ctx.bel(bels::OBUF[i]);
        let mode = "OBUF";
        bctx.test_manual("ENABLE", "1")
            .mode(mode)
            .attr("ENABLE_MISR", "FALSE")
            .prop(IobExtra::new(Dir::W))
            .prop(IobExtra::new(Dir::E))
            .prop(IobExtra::new(Dir::S))
            .prop(IobExtra::new(Dir::N))
            .commit();
        bctx.mode(mode)
            .prop(IobExtra::new(Dir::W))
            .prop(IobExtra::new(Dir::E))
            .prop(IobExtra::new(Dir::S))
            .prop(IobExtra::new(Dir::N))
            .test_manual("ENABLE_MISR", "TRUE")
            .attr_diff("ENABLE_MISR", "FALSE", "TRUE")
            .commit();
        for pin in ["CLK", "CE", "SR", "REV", "O"] {
            bctx.mode(mode)
                .attr("OMUX", "OFF")
                .attr("OFF", "#FF")
                .test_inv(pin);
        }
        bctx.mode(mode)
            .attr("OINV", "O")
            .attr("OFF_INIT_ATTR", "INIT1")
            .attr("CEINV", "CE_B")
            .pin("O")
            .pin("CE")
            .test_enum("OFF", &["#FF", "#LATCH"]);
        bctx.mode(mode)
            .attr("OFF", "#FF")
            .attr("OINV", "O")
            .pin("O")
            .test_enum("OFFATTRBOX", &["SYNC", "ASYNC"]);
        bctx.mode(mode)
            .attr("OFF", "#FF")
            .attr("OINV", "O")
            .attr("OFF_SR_ATTR", "SRLOW")
            .pin("O")
            .test_enum("OFF_INIT_ATTR", &["INIT0", "INIT1"]);
        bctx.mode(mode)
            .attr("OFF", "#FF")
            .attr("OINV", "O")
            .attr("OFF_INIT_ATTR", "INIT0")
            .pin("O")
            .test_enum("OFF_SR_ATTR", &["SRLOW", "SRHIGH"]);
        bctx.mode(mode)
            .attr("OFF", "#FF")
            .attr("OINV", "O")
            .pin("O")
            .test_enum("OMUX", &["O", "OFF"]);
    }
}

pub fn collect_fuzzers(ctx: &mut CollectorCtx) {
    for i in 0..4 {
        let tile = "IOI.FC";
        let bel = &format!("IBUF{i}");
        ctx.state.get_diff(tile, bel, "ENABLE", "1").assert_empty();
        ctx.state
            .get_diff(tile, bel, "ENABLE_O2IPADPATH", "1")
            .assert_empty();
        let diff_i = ctx.state.get_diff(tile, bel, "ENABLE_O2IPATH", "1");
        let diff_iq = ctx.state.get_diff(tile, bel, "ENABLE_O2IQPATH", "1");
        let (diff_i, diff_iq, diff_common) = Diff::split(diff_i, diff_iq);
        ctx.tiledb
            .insert(tile, bel, "ENABLE_O2IPATH", xlat_bit(diff_i));
        ctx.tiledb
            .insert(tile, bel, "ENABLE_O2IQPATH", xlat_bit(diff_iq));
        ctx.tiledb
            .insert(tile, bel, "ENABLE_O2I_O2IQ_PATH", xlat_bit(diff_common));
        for pin in ["CLK", "CE"] {
            ctx.collect_inv(tile, bel, pin);
        }
        for pin in ["REV", "SR"] {
            let d0 = ctx.state.get_diff(tile, bel, format!("{pin}INV"), pin);
            let d1 = ctx
                .state
                .get_diff(tile, bel, format!("{pin}INV"), format!("{pin}_B"));
            let (d0, d1, de) = Diff::split(d0, d1);
            ctx.tiledb
                .insert(tile, bel, format!("INV.{pin}"), xlat_bool(d0, d1));
            ctx.tiledb
                .insert(tile, bel, format!("FF_{pin}_ENABLE"), xlat_bit(de));
        }
        ctx.state.get_diff(tile, bel, "IMUX", "1").assert_empty();
        ctx.state.get_diff(tile, bel, "IFFDMUX", "1").assert_empty();
        let diff_i = ctx.state.get_diff(tile, bel, "IMUX", "0");
        let diff_iff = ctx.state.get_diff(tile, bel, "IFFDMUX", "0");
        let (diff_i, diff_iff, diff_common) = Diff::split(diff_i, diff_iff);
        ctx.tiledb
            .insert(tile, bel, "I_DELAY_ENABLE", xlat_bit(diff_i));
        ctx.tiledb
            .insert(tile, bel, "IFF_DELAY_ENABLE", xlat_bit(diff_iff));
        ctx.tiledb
            .insert(tile, bel, "DELAY_ENABLE", xlat_bit_wide(diff_common));
        let item = ctx.extract_enum_bool(tile, bel, "IFF", "#FF", "#LATCH");
        ctx.tiledb.insert(tile, bel, "FF_LATCH", item);
        let item = ctx.extract_enum_bool(tile, bel, "IFFATTRBOX", "ASYNC", "SYNC");
        ctx.tiledb.insert(tile, bel, "FF_SR_SYNC", item);
        let item = ctx.extract_enum_bool(tile, bel, "IFF_INIT_ATTR", "INIT0", "INIT1");
        ctx.tiledb.insert(tile, bel, "FF_INIT", item);
        let item = ctx.extract_enum_bool(tile, bel, "IFF_SR_ATTR", "SRLOW", "SRHIGH");
        ctx.tiledb.insert(tile, bel, "FF_SRVAL", item);
        ctx.tiledb.insert(
            tile,
            bel,
            "READBACK_I",
            TileItem::from_bit(TileBit::new(0, 3, [0, 31, 32, 63][i]), false),
        );
        for tile in ["IOBS.FC.B", "IOBS.FC.T", "IOBS.FC.L", "IOBS.FC.R"] {
            ctx.collect_bit(tile, bel, "ENABLE", "1");
            ctx.collect_bit(tile, bel, "ENABLE_O2IPADPATH", "1");
        }
    }
    for i in 0..4 {
        let tile = "IOI.FC";
        let bel = &format!("OBUF{i}");
        ctx.state.get_diff(tile, bel, "ENABLE", "1").assert_empty();
        ctx.state
            .get_diff(tile, bel, "ENABLE_MISR", "TRUE")
            .assert_empty();
        for pin in ["CLK", "O"] {
            ctx.collect_inv(tile, bel, pin);
        }
        ctx.collect_int_inv(&["INT.IOI.FC"], tile, bel, "CE", false);
        for pin in ["REV", "SR"] {
            let d0 = ctx.state.get_diff(tile, bel, format!("{pin}INV"), pin);
            let d1 = ctx
                .state
                .get_diff(tile, bel, format!("{pin}INV"), format!("{pin}_B"));
            let (d0, d1, de) = Diff::split(d0, d1);
            if pin == "REV" {
                ctx.tiledb
                    .insert(tile, bel, format!("INV.{pin}"), xlat_bool(d0, d1));
            } else {
                ctx.insert_int_inv(&["INT.IOI.FC"], tile, bel, pin, xlat_bool(d0, d1));
            }
            ctx.tiledb
                .insert(tile, bel, format!("FF_{pin}_ENABLE"), xlat_bit(de));
        }
        let item = ctx.extract_enum_bool(tile, bel, "OFF", "#FF", "#LATCH");
        ctx.tiledb.insert(tile, bel, "FF_LATCH", item);
        let item = ctx.extract_enum_bool(tile, bel, "OFFATTRBOX", "ASYNC", "SYNC");
        ctx.tiledb.insert(tile, bel, "FF_SR_SYNC", item);
        let item = ctx.extract_enum_bool(tile, bel, "OFF_INIT_ATTR", "INIT0", "INIT1");
        ctx.tiledb.insert(tile, bel, "FF_INIT", item);
        let item = ctx.extract_enum_bool(tile, bel, "OFF_SR_ATTR", "SRLOW", "SRHIGH");
        ctx.tiledb.insert(tile, bel, "FF_SRVAL", item);
        ctx.collect_enum_default(tile, bel, "OMUX", &["O", "OFF"], "NONE");
        for tile in ["IOBS.FC.B", "IOBS.FC.T", "IOBS.FC.L", "IOBS.FC.R"] {
            ctx.collect_bit_wide(tile, bel, "ENABLE", "1");
            ctx.collect_bit(tile, bel, "ENABLE_MISR", "TRUE");
        }
    }
}
