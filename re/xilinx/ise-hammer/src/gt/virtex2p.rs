use bitvec::prelude::*;
use prjcombine_interconnect::db::{BelId, PinDir};
use prjcombine_re_collector::OcdMode;
use prjcombine_re_hammer::Session;
use unnamed_entity::EntityId;

use crate::{
    backend::IseBackend, diff::CollectorCtx, fgen::TileBits, fuzz::FuzzCtx, fuzz_enum, fuzz_inv,
    fuzz_multi_attr_bin, fuzz_multi_attr_dec, fuzz_one,
};

pub fn add_fuzzers<'a>(session: &mut Session<IseBackend<'a>>, backend: &IseBackend<'a>) {
    let intdb = backend.egrid.db;
    for tile in ["GIGABIT.B", "GIGABIT.T"] {
        let ctx = FuzzCtx::new(session, backend, tile, "GT", TileBits::MainAuto);
        fuzz_one!(ctx, "ENABLE", "1", [], [(mode "GT")]);
        let bel_data = &intdb.nodes[ctx.node_kind].bels[ctx.bel];
        for (pin, pin_data) in &bel_data.pins {
            if pin_data.dir != PinDir::Input {
                continue;
            }
            assert_eq!(pin_data.wires.len(), 1);
            let wire = *pin_data.wires.first().unwrap();
            if intdb.wires.key(wire.1).starts_with("IMUX.G") {
                continue;
            }
            fuzz_inv!(ctx, pin, [(mode "GT")]);
        }
        fuzz_enum!(ctx, "IOSTANDARD", [
            "FIBRE_CHAN", "ETHERNET", "XAUI", "INFINIBAND", "AURORA"
        ], [(mode "GT")]);
        for attr in [
            "ALIGN_COMMA_MSB",
            "PCOMMA_DETECT",
            "MCOMMA_DETECT",
            "DEC_PCOMMA_DETECT",
            "DEC_MCOMMA_DETECT",
            "DEC_VALID_COMMA_ONLY",
            "SERDES_10B",
            "RX_DECODE_USE",
            "RX_BUFFER_USE",
            "TX_BUFFER_USE",
            "CLK_CORRECT_USE",
            "CLK_COR_KEEP_IDLE",
            "CLK_COR_SEQ_2_USE",
            "CLK_COR_INSERT_IDLE_FLAG",
            "CHAN_BOND_SEQ_2_USE",
            "CHAN_BOND_ONE_SHOT",
            "TEST_MODE_1",
            "TEST_MODE_2",
            "TEST_MODE_3",
            "TEST_MODE_4",
            "TEST_MODE_5",
            "TEST_MODE_6",
            "RX_LOSS_OF_SYNC_FSM",
            "TX_CRC_USE",
            "RX_CRC_USE",
        ] {
            fuzz_enum!(ctx, attr, ["FALSE", "TRUE"], [(mode "GT")]);
        }
        fuzz_enum!(ctx, "TX_PREEMPHASIS", ["0", "1", "2", "3"], [(mode "GT")]);
        fuzz_enum!(ctx, "TERMINATION_IMP", ["50", "75"], [(mode "GT")]);
        fuzz_enum!(ctx, "CLK_COR_SEQ_LEN", ["1", "2", "3", "4"], [(mode "GT")]);
        fuzz_multi_attr_dec!(ctx, "CLK_COR_REPEAT_WAIT", 5, [(mode "GT")]);
        fuzz_enum!(ctx, "CHAN_BOND_MODE", ["MASTER", "SLAVE_1_HOP", "SLAVE_2_HOPS"], [(mode "GT")]);
        fuzz_enum!(ctx, "CHAN_BOND_WAIT", ["1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15"], [(mode "GT")]);
        fuzz_enum!(ctx, "CHAN_BOND_LIMIT", ["1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15", "16", "17", "18", "19", "20", "21", "22", "23", "24", "25", "26", "27", "28", "29", "30", "31"], [(mode "GT")]);
        fuzz_enum!(ctx, "CHAN_BOND_SEQ_LEN", ["1", "2", "3", "4"], [(mode "GT")]);
        fuzz_multi_attr_dec!(ctx, "CHAN_BOND_OFFSET", 4, [(mode "GT")]);
        fuzz_enum!(ctx, "RX_DATA_WIDTH", ["1", "2", "4"], [(mode "GT")]);
        fuzz_enum!(ctx, "TX_DATA_WIDTH", ["1", "2", "4"], [(mode "GT")]);
        fuzz_multi_attr_dec!(ctx, "RX_BUFFER_LIMIT", 4, [(mode "GT"), (attr "CHAN_BOND_MODE", "")]);
        fuzz_one!(ctx, "RX_BUFFER_LIMIT", "15.MASTER", [
            (mode "GT"),
            (attr "CHAN_BOND_MODE", "MASTER")
        ], [
            (attr "RX_BUFFER_LIMIT", "15")
        ]);
        fuzz_one!(ctx, "RX_BUFFER_LIMIT", "15.SLAVE_1_HOP", [
            (mode "GT"),
            (attr "CHAN_BOND_MODE", "SLAVE_1_HOP")
        ], [
            (attr "RX_BUFFER_LIMIT", "15")
        ]);
        fuzz_one!(ctx, "RX_BUFFER_LIMIT", "15.SLAVE_2_HOPS", [
            (mode "GT"),
            (attr "CHAN_BOND_MODE", "SLAVE_2_HOPS")
        ], [
            (attr "RX_BUFFER_LIMIT", "15")
        ]);
        fuzz_enum!(ctx, "RX_LOS_INVALID_INCR", ["1", "2", "4", "8", "16", "32", "64", "128"], [(mode "GT")]);
        fuzz_enum!(ctx, "RX_LOS_THRESHOLD", ["4", "8", "16", "32", "64", "128", "256", "512"], [(mode "GT")]);
        fuzz_enum!(ctx, "CRC_FORMAT", ["USER_MODE", "ETHERNET", "INFINIBAND", "FIBRE_CHAN"], [(mode "GT")]);
        fuzz_enum!(ctx, "CRC_START_OF_PKT", ["K28_0", "K28_1", "K28_2", "K28_3", "K28_4", "K28_5", "K28_6", "K28_7", "K23_7", "K27_7", "K29_7", "K30_7"], [(mode "GT")]);
        fuzz_enum!(ctx, "CRC_END_OF_PKT", ["K28_0", "K28_1", "K28_2", "K28_3", "K28_4", "K28_5", "K28_6", "K28_7", "K23_7", "K27_7", "K29_7", "K30_7"], [(mode "GT")]);
        fuzz_enum!(ctx, "TX_DIFF_CTRL", ["400", "500", "600", "700", "800"], [(mode "GT")]);
        fuzz_enum!(ctx, "REF_CLK_V_SEL", ["0", "1"], [(mode "GT")]);
        fuzz_multi_attr_bin!(ctx, "TX_CRC_FORCE_VALUE", 8, [(mode "GT")]);
        fuzz_multi_attr_bin!(ctx, "COMMA_10B_MASK", 10, [(mode "GT")]);
        fuzz_multi_attr_bin!(ctx, "MCOMMA_10B_VALUE", 10, [(mode "GT")]);
        fuzz_multi_attr_bin!(ctx, "PCOMMA_10B_VALUE", 10, [(mode "GT")]);
        fuzz_multi_attr_bin!(ctx, "CLK_COR_SEQ_1_1", 11, [(mode "GT")]);
        fuzz_multi_attr_bin!(ctx, "CLK_COR_SEQ_1_2", 11, [(mode "GT")]);
        fuzz_multi_attr_bin!(ctx, "CLK_COR_SEQ_1_3", 11, [(mode "GT")]);
        fuzz_multi_attr_bin!(ctx, "CLK_COR_SEQ_1_4", 11, [(mode "GT")]);
        fuzz_multi_attr_bin!(ctx, "CLK_COR_SEQ_2_1", 11, [(mode "GT")]);
        fuzz_multi_attr_bin!(ctx, "CLK_COR_SEQ_2_2", 11, [(mode "GT")]);
        fuzz_multi_attr_bin!(ctx, "CLK_COR_SEQ_2_3", 11, [(mode "GT")]);
        fuzz_multi_attr_bin!(ctx, "CLK_COR_SEQ_2_4", 11, [(mode "GT")]);
        fuzz_multi_attr_bin!(ctx, "CHAN_BOND_SEQ_1_1", 11, [(mode "GT")]);
        fuzz_multi_attr_bin!(ctx, "CHAN_BOND_SEQ_1_2", 11, [(mode "GT")]);
        fuzz_multi_attr_bin!(ctx, "CHAN_BOND_SEQ_1_3", 11, [(mode "GT")]);
        fuzz_multi_attr_bin!(ctx, "CHAN_BOND_SEQ_1_4", 11, [(mode "GT")]);
        fuzz_multi_attr_bin!(ctx, "CHAN_BOND_SEQ_2_1", 11, [(mode "GT")]);
        fuzz_multi_attr_bin!(ctx, "CHAN_BOND_SEQ_2_2", 11, [(mode "GT")]);
        fuzz_multi_attr_bin!(ctx, "CHAN_BOND_SEQ_2_3", 11, [(mode "GT")]);
        fuzz_multi_attr_bin!(ctx, "CHAN_BOND_SEQ_2_4", 11, [(mode "GT")]);
    }
}

pub fn collect_fuzzers(ctx: &mut CollectorCtx) {
    let egrid = ctx.edev.egrid();
    for tile in ["GIGABIT.B", "GIGABIT.T"] {
        let node_kind = egrid.db.get_node(tile);
        let bel = "GT";
        ctx.collect_bit(tile, bel, "ENABLE", "1");
        let bel_data = &egrid.db.nodes[node_kind].bels[BelId::from_idx(0)];
        for (pin, pin_data) in &bel_data.pins {
            if pin_data.dir != PinDir::Input {
                continue;
            }
            assert_eq!(pin_data.wires.len(), 1);
            let wire = *pin_data.wires.first().unwrap();
            if egrid.db.wires.key(wire.1).starts_with("IMUX.G") {
                continue;
            }
            let int_tiles = &["INT.GT.CLKPAD", "INT.PPC", "INT.PPC", "INT.PPC", "INT.PPC"];
            let flip = egrid.db.wires.key(wire.1).starts_with("IMUX.SR");
            ctx.collect_int_inv(int_tiles, tile, bel, pin, flip);
        }
        for attr in [
            "ALIGN_COMMA_MSB",
            "PCOMMA_DETECT",
            "MCOMMA_DETECT",
            "DEC_PCOMMA_DETECT",
            "DEC_MCOMMA_DETECT",
            "DEC_VALID_COMMA_ONLY",
            "SERDES_10B",
            "RX_DECODE_USE",
            "RX_BUFFER_USE",
            "TX_BUFFER_USE",
            "CLK_CORRECT_USE",
            "CLK_COR_KEEP_IDLE",
            "CLK_COR_SEQ_2_USE",
            "CLK_COR_INSERT_IDLE_FLAG",
            "CHAN_BOND_SEQ_2_USE",
            "CHAN_BOND_ONE_SHOT",
            "TEST_MODE_1",
            "TEST_MODE_2",
            "TEST_MODE_3",
            "TEST_MODE_4",
            "TEST_MODE_5",
            "TEST_MODE_6",
            "RX_LOSS_OF_SYNC_FSM",
            "TX_CRC_USE",
            "RX_CRC_USE",
        ] {
            ctx.collect_enum_bool(tile, bel, attr, "FALSE", "TRUE");
        }
        for val in ["ETHERNET", "AURORA", "FIBRE_CHAN", "INFINIBAND", "XAUI"] {
            ctx.state
                .get_diff(tile, bel, "IOSTANDARD", val)
                .assert_empty();
        }
        ctx.collect_enum_int(tile, bel, "TX_PREEMPHASIS", 0..4, 0);
        ctx.collect_enum(tile, bel, "TERMINATION_IMP", &["50", "75"]);
        ctx.collect_enum(tile, bel, "CLK_COR_SEQ_LEN", &["1", "2", "3", "4"]);
        ctx.collect_enum(tile, bel, "CHAN_BOND_SEQ_LEN", &["1", "2", "3", "4"]);
        ctx.collect_bitvec(tile, bel, "CLK_COR_REPEAT_WAIT", "");
        ctx.collect_enum_default(
            tile,
            bel,
            "CHAN_BOND_MODE",
            &["MASTER", "SLAVE_1_HOP", "SLAVE_2_HOPS"],
            "NONE",
        );
        ctx.collect_enum_int(tile, bel, "CHAN_BOND_WAIT", 1..16, 0);
        ctx.collect_enum_int(tile, bel, "CHAN_BOND_LIMIT", 1..32, 0);
        ctx.collect_bitvec(tile, bel, "CHAN_BOND_OFFSET", "");
        ctx.collect_enum(tile, bel, "RX_DATA_WIDTH", &["1", "2", "4"]);
        ctx.collect_enum(tile, bel, "TX_DATA_WIDTH", &["1", "2", "4"]);
        ctx.collect_bitvec(tile, bel, "RX_BUFFER_LIMIT", "");
        let item = ctx.collector.tiledb.item(tile, bel, "RX_BUFFER_LIMIT");
        for (name, val) in [
            ("15.MASTER", bitvec![0, 0, 1, 1]),
            ("15.SLAVE_1_HOP", bitvec![0, 0, 1, 0]),
            ("15.SLAVE_2_HOPS", bitvec![0, 0, 1, 0]),
        ] {
            let mut diff = ctx
                .collector
                .state
                .get_diff(tile, bel, "RX_BUFFER_LIMIT", name);
            diff.apply_bitvec_diff(item, &val, &BitVec::repeat(false, 4));
            diff.assert_empty();
        }
        ctx.collect_enum(
            tile,
            bel,
            "RX_LOS_INVALID_INCR",
            &["1", "2", "4", "8", "16", "32", "64", "128"],
        );
        ctx.collect_enum(
            tile,
            bel,
            "RX_LOS_THRESHOLD",
            &["4", "8", "16", "32", "64", "128", "256", "512"],
        );
        ctx.collect_enum(
            tile,
            bel,
            "CRC_FORMAT",
            &["USER_MODE", "ETHERNET", "INFINIBAND", "FIBRE_CHAN"],
        );
        ctx.collect_enum_ocd(
            tile,
            bel,
            "CRC_START_OF_PKT",
            &[
                "K28_0", "K28_1", "K28_2", "K28_3", "K28_4", "K28_5", "K28_6", "K28_7", "K23_7",
                "K27_7", "K29_7", "K30_7",
            ],
            OcdMode::BitOrder,
        );
        ctx.collect_enum_ocd(
            tile,
            bel,
            "CRC_END_OF_PKT",
            &[
                "K28_0", "K28_1", "K28_2", "K28_3", "K28_4", "K28_5", "K28_6", "K28_7", "K23_7",
                "K27_7", "K29_7", "K30_7",
            ],
            OcdMode::BitOrder,
        );
        ctx.collect_enum(
            tile,
            bel,
            "TX_DIFF_CTRL",
            &["400", "500", "600", "700", "800"],
        );
        ctx.collect_enum_bool(tile, bel, "REF_CLK_V_SEL", "0", "1");
        ctx.collect_bitvec(tile, bel, "TX_CRC_FORCE_VALUE", "");
        ctx.collect_bitvec(tile, bel, "COMMA_10B_MASK", "");
        ctx.collect_bitvec(tile, bel, "MCOMMA_10B_VALUE", "");
        ctx.collect_bitvec(tile, bel, "PCOMMA_10B_VALUE", "");
        ctx.collect_bitvec(tile, bel, "CLK_COR_SEQ_1_1", "");
        ctx.collect_bitvec(tile, bel, "CLK_COR_SEQ_1_2", "");
        ctx.collect_bitvec(tile, bel, "CLK_COR_SEQ_1_3", "");
        ctx.collect_bitvec(tile, bel, "CLK_COR_SEQ_1_4", "");
        ctx.collect_bitvec(tile, bel, "CLK_COR_SEQ_2_1", "");
        ctx.collect_bitvec(tile, bel, "CLK_COR_SEQ_2_2", "");
        ctx.collect_bitvec(tile, bel, "CLK_COR_SEQ_2_3", "");
        ctx.collect_bitvec(tile, bel, "CLK_COR_SEQ_2_4", "");
        ctx.collect_bitvec(tile, bel, "CHAN_BOND_SEQ_1_1", "");
        ctx.collect_bitvec(tile, bel, "CHAN_BOND_SEQ_1_2", "");
        ctx.collect_bitvec(tile, bel, "CHAN_BOND_SEQ_1_3", "");
        ctx.collect_bitvec(tile, bel, "CHAN_BOND_SEQ_1_4", "");
        ctx.collect_bitvec(tile, bel, "CHAN_BOND_SEQ_2_1", "");
        ctx.collect_bitvec(tile, bel, "CHAN_BOND_SEQ_2_2", "");
        ctx.collect_bitvec(tile, bel, "CHAN_BOND_SEQ_2_3", "");
        ctx.collect_bitvec(tile, bel, "CHAN_BOND_SEQ_2_4", "");
    }
}
