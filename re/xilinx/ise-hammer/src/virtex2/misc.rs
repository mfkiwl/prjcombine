use std::collections::HashSet;

use prjcombine_interconnect::{
    dir::DirHV,
    grid::{CellCoord, DieId, TileCoord},
};
use prjcombine_re_fpga_hammer::{
    Diff, FuzzerProp, OcdMode, concat_bitvec, extract_bitvec_val, extract_bitvec_val_part,
    xlat_bit, xlat_bit_wide, xlat_bitvec, xlat_enum, xlat_enum_ocd,
};
use prjcombine_re_hammer::{Fuzzer, Session};
use prjcombine_re_xilinx_geom::{ExpandedBond, ExpandedDevice, ExpandedNamedDevice};
use prjcombine_types::{
    bits,
    bittile::BitTile as _,
    bitvec::BitVec,
    bsdata::{TileBit, TileItem, TileItemKind},
};
use prjcombine_virtex2::{
    bels,
    chip::{ChipKind, IoDiffKind},
    iob::IobKind,
    tslots,
};
use prjcombine_xilinx_bitstream::{BitTile, Reg};
use unnamed_entity::EntityId;

use crate::{
    backend::{IseBackend, Key, MultiValue},
    collector::CollectorCtx,
    generic::{
        fbuild::{FuzzBuilderBase, FuzzCtx},
        iostd::{DciKind, DiffKind},
        props::{DynProp, extra::ExtraReg, pip::PinFar, relation::TileRelation},
    },
    virtex2::io::get_iostds,
};

#[derive(Copy, Clone, Debug)]
struct IntRelation;

impl TileRelation for IntRelation {
    fn resolve(&self, _backend: &IseBackend, tcrd: TileCoord) -> Option<TileCoord> {
        Some(tcrd.tile(tslots::INT))
    }
}

#[derive(Clone, Debug)]
struct ForceBits(Vec<BitTile>);

impl<'b> FuzzerProp<'b, IseBackend<'b>> for ForceBits {
    fn dyn_clone(&self) -> Box<DynProp<'b>> {
        Box::new(Clone::clone(self))
    }

    fn apply<'a>(
        &self,
        _backend: &IseBackend<'a>,
        _tcrd: TileCoord,
        mut fuzzer: Fuzzer<IseBackend<'a>>,
    ) -> Option<(Fuzzer<IseBackend<'a>>, bool)> {
        fuzzer.info.features[0].tiles = self.0.clone();
        Some((fuzzer, false))
    }
}

pub fn add_fuzzers<'a>(
    session: &mut Session<'a, IseBackend<'a>>,
    backend: &'a IseBackend<'a>,
    skip_io: bool,
    devdata_only: bool,
) {
    let ExpandedDevice::Virtex2(edev) = backend.edev else {
        unreachable!()
    };
    let ExpandedNamedDevice::Virtex2(endev) = backend.endev else {
        unreachable!()
    };

    let (ll, ul, lr, ur) = match edev.chip.kind {
        ChipKind::Virtex2 => ("LL.V2", "UL.V2", "LR.V2", "UR.V2"),
        ChipKind::Virtex2P | ChipKind::Virtex2PX => ("LL.V2P", "UL.V2P", "LR.V2P", "UR.V2P"),
        ChipKind::Spartan3 => ("LL.S3", "UL.S3", "LR.S3", "UR.S3"),
        ChipKind::FpgaCore => ("LL.FC", "UL.FC", "LR.FC", "UR.FC"),
        ChipKind::Spartan3E => ("LL.S3E", "UL.S3E", "LR.S3E", "UR.S3E"),
        ChipKind::Spartan3A | ChipKind::Spartan3ADsp => ("LL.S3A", "UL.S3A", "LR.S3A", "UR.S3A"),
    };

    let freeze_dci_btiles = vec![
        edev.btile_lrterm(edev.chip.corner(DirHV::SW).cell),
        edev.btile_btterm(edev.chip.corner(DirHV::SW).cell),
        edev.btile_lrterm(edev.chip.corner(DirHV::SW).cell)
            .to_fixup(),
        edev.btile_btterm(edev.chip.corner(DirHV::SW).cell)
            .to_fixup(),
        BitTile::Reg(DieId::from_idx(0), Reg::FakeFreezeDciNops),
        BitTile::RegPresent(DieId::from_idx(0), Reg::FakeFreezeDciNops),
    ];

    if devdata_only {
        let mut ctx = FuzzCtx::new(session, backend, ll);
        if !edev.chip.kind.is_virtex2() {
            for (attr, vals) in [
                ("SEND_VGG0", &["1", "0"][..]),
                ("SEND_VGG1", &["1", "0"][..]),
                ("SEND_VGG2", &["1", "0"][..]),
                ("SEND_VGG3", &["1", "0"][..]),
                ("VGG_SENDMAX", &["YES", "NO"][..]),
            ] {
                for &val in vals {
                    let mut builder = ctx.build();
                    if edev.chip.kind.is_spartan3a() {
                        builder = builder.extra_tile_reg_attr(
                            Reg::Cor1,
                            "REG.COR1.S3A",
                            "MISC",
                            attr,
                            val,
                        );
                    }
                    builder
                        .test_manual("MISC", attr, val)
                        .global(attr, val)
                        .commit();
                }
            }
        }
        if edev.chip.kind.is_virtex2() {
            let mut ctx = FuzzCtx::new(session, backend, ll);
            ctx.build()
                .prop(ForceBits(freeze_dci_btiles))
                .global_mutex("DCI", "FREEZE")
                .no_global("ENCRYPT")
                .test_manual("MISC", "FREEZE_DCI", "1")
                .global("FREEZEDCI", "YES")
                .commit();
        }

        return;
    }

    let reg_cor = if edev.chip.kind.is_virtex2() {
        "REG.COR"
    } else if edev.chip.kind == ChipKind::Spartan3 {
        "REG.COR.S3"
    } else if edev.chip.kind == ChipKind::FpgaCore {
        "REG.COR.FC"
    } else {
        "REG.COR.S3E"
    };

    fn fuzz_global(
        ctx: &mut FuzzCtx,
        bel: &'static str,
        attr: &'static str,
        vals: &'static [&'static str],
    ) {
        for &val in vals {
            ctx.test_manual(bel, attr, val).global(attr, val).commit();
        }
    }
    fn fuzz_pull(ctx: &mut FuzzCtx, bel: &'static str, attr: &'static str) {
        fuzz_global(ctx, bel, attr, &["PULLNONE", "PULLDOWN", "PULLUP"]);
    }

    if edev.chip.kind == ChipKind::Spartan3 {
        for tile in [ll, ul, lr, ur] {
            let mut ctx = FuzzCtx::new(session, backend, tile);
            for bel in [bels::DCIRESET0, bels::DCIRESET1] {
                let mut bctx = ctx.bel(bel);
                bctx.test_manual("PRESENT", "1").mode("DCIRESET").commit();
            }
        }
    }

    // LL
    {
        let mut ctx = FuzzCtx::new(session, backend, ll);
        // MISC
        if edev.chip.kind.is_virtex2() {
            fuzz_global(&mut ctx, "MISC", "DISABLEBANDGAP", &["YES", "NO"]);
            fuzz_global(&mut ctx, "MISC", "RAISEVGG", &["YES", "NO"]);
            fuzz_global(&mut ctx, "MISC", "IBCLK_N2", &["1", "0"]);
            fuzz_global(&mut ctx, "MISC", "IBCLK_N4", &["1", "0"]);
            fuzz_global(&mut ctx, "MISC", "IBCLK_N8", &["1", "0"]);
            fuzz_global(&mut ctx, "MISC", "IBCLK_N16", &["1", "0"]);
            fuzz_global(&mut ctx, "MISC", "IBCLK_N32", &["1", "0"]);
            for attr in ["ZCLK_N2", "ZCLK_N4", "ZCLK_N8", "ZCLK_N16", "ZCLK_N32"] {
                for val in ["1", "0"] {
                    ctx.build()
                        .global_mutex("DCI", "NO")
                        .test_manual("MISC", attr, val)
                        .global(attr, val)
                        .commit();
                }
            }
            if edev.chip.kind.is_virtex2p() {
                fuzz_global(&mut ctx, "MISC", "DISABLEVGGGENERATION", &["YES", "NO"]);
            }
        } else {
            for (attr, vals) in [
                ("SEND_VGG0", &["1", "0"][..]),
                ("SEND_VGG1", &["1", "0"][..]),
                ("SEND_VGG2", &["1", "0"][..]),
                ("SEND_VGG3", &["1", "0"][..]),
                ("VGG_SENDMAX", &["YES", "NO"][..]),
                ("VGG_ENABLE_OFFCHIP", &["YES", "NO"][..]),
            ] {
                for &val in vals {
                    let mut builder = ctx.build();
                    if edev.chip.kind.is_spartan3a() {
                        builder = builder.extra_tile_reg_attr(
                            Reg::Cor1,
                            "REG.COR1.S3A",
                            "MISC",
                            attr,
                            val,
                        );
                    }
                    builder
                        .test_manual("MISC", attr, val)
                        .global(attr, val)
                        .commit();
                }
            }
        }
        if edev.chip.kind == ChipKind::Spartan3 {
            fuzz_global(&mut ctx, "MISC", "GATE_GHIGH", &["YES", "NO"]);
            fuzz_global(&mut ctx, "MISC", "IDCI_OSC_SEL0", &["1", "0"]);
            fuzz_global(&mut ctx, "MISC", "IDCI_OSC_SEL1", &["1", "0"]);
            fuzz_global(&mut ctx, "MISC", "IDCI_OSC_SEL2", &["1", "0"]);
        }
        if edev.chip.kind.is_spartan3ea() {
            fuzz_global(
                &mut ctx,
                "MISC",
                "TEMPSENSOR",
                &["NONE", "PGATE", "CGATE", "BG", "THERM"],
            );
        }
        if edev.chip.kind.is_spartan3a() {
            fuzz_pull(&mut ctx, "MISC", "CCLK2PIN");
            fuzz_pull(&mut ctx, "MISC", "MOSI2PIN");
        } else if edev.chip.kind != ChipKind::Spartan3E && edev.chip.kind != ChipKind::FpgaCore {
            fuzz_pull(&mut ctx, "MISC", "M0PIN");
            fuzz_pull(&mut ctx, "MISC", "M1PIN");
            fuzz_pull(&mut ctx, "MISC", "M2PIN");
        }
        if edev.chip.kind.is_virtex2() {
            ctx.build()
                .prop(ForceBits(freeze_dci_btiles))
                .global_mutex("DCI", "FREEZE")
                .no_global("ENCRYPT")
                .test_manual("MISC", "FREEZE_DCI", "1")
                .global("FREEZEDCI", "YES")
                .commit();
        }
    }

    // UL
    {
        let mut ctx = FuzzCtx::new(session, backend, ul);
        if edev.chip.kind != ChipKind::FpgaCore {
            fuzz_global(&mut ctx, "MISC", "PROGPIN", &["PULLUP", "PULLNONE"]);
            fuzz_pull(&mut ctx, "MISC", "TDIPIN");
        }
        if edev.chip.kind.is_spartan3a() {
            fuzz_pull(&mut ctx, "MISC", "TMSPIN");
        }
        if !edev.chip.kind.is_spartan3ea() && edev.chip.kind != ChipKind::FpgaCore {
            fuzz_pull(&mut ctx, "MISC", "HSWAPENPIN");
        }
        for val in ["NO", "YES"] {
            let mut builder = ctx.build();
            if edev.chip.kind.is_virtex2() {
                let cnr_ne = edev.chip.corner(DirHV::NE);
                builder = builder.extra_tile_fixed(cnr_ne, "MISC");
            }
            builder
                .test_manual("MISC", "TEST_LL", val)
                .global("TESTLL", val)
                .commit();
        }

        let mut bctx = ctx.bel(bels::PMV);
        bctx.build()
            .test_manual("PRESENT", "1")
            .mode("PMV")
            .commit();
        if edev.chip.kind.is_spartan3a() {
            let mut bctx = ctx.bel(bels::DNA_PORT);
            bctx.build()
                .test_manual("PRESENT", "1")
                .mode("DNA_PORT")
                .commit();
        }
    }

    {
        // LR
        let mut ctx = FuzzCtx::new(session, backend, lr);
        if edev.chip.kind != ChipKind::FpgaCore {
            fuzz_global(&mut ctx, "MISC", "DONEPIN", &["PULLUP", "PULLNONE"]);
        }
        if !edev.chip.kind.is_spartan3a() && edev.chip.kind != ChipKind::FpgaCore {
            fuzz_global(&mut ctx, "MISC", "CCLKPIN", &["PULLUP", "PULLNONE"]);
        }
        if edev.chip.kind.is_virtex2() {
            fuzz_global(&mut ctx, "MISC", "POWERDOWNPIN", &["PULLUP", "PULLNONE"]);
        }
        if edev.chip.kind == ChipKind::FpgaCore {
            for attr in ["ABUFF0", "ABUFF1", "ABUFF2", "ABUFF3"] {
                for val in ["0", "1"] {
                    ctx.test_manual("MISC", attr, val)
                        .global(attr, val)
                        .commit();
                }
            }
        }

        let mut bctx = ctx.bel(bels::STARTUP);
        bctx.test_manual("PRESENT", "1").mode("STARTUP").commit();
        bctx.mode("STARTUP")
            .null_bits()
            .extra_tile(IntRelation, "STARTUP")
            .global("STARTUPCLK", "JTAGCLK")
            .test_inv("CLK");
        bctx.mode("STARTUP")
            .extra_tile(IntRelation, "STARTUP")
            .no_pin("GSR")
            .test_inv("GTS");
        bctx.mode("STARTUP")
            .extra_tile(IntRelation, "STARTUP")
            .no_pin("GTS")
            .test_inv("GSR");
        for attr in ["GTS_SYNC", "GSR_SYNC", "GWE_SYNC"] {
            if !edev.chip.kind.is_virtex2() && attr == "GWE_SYNC" {
                continue;
            }
            for val in ["NO", "YES"] {
                bctx.mode("STARTUP")
                    .test_manual(attr, val)
                    .global(attr, val)
                    .commit();
            }
        }
        let (reg, reg_name) = if edev.chip.kind.is_spartan3a() {
            (Reg::Cor1, "REG.COR1.S3A")
        } else {
            (Reg::Cor0, reg_cor)
        };
        if edev.chip.kind == ChipKind::Spartan3E {
            bctx.mode("STARTUP")
                .null_bits()
                .extra_tile_reg_attr(reg, reg_name, "STARTUP", "MULTIBOOT_ENABLE", "1")
                .test_manual("MULTIBOOT_ENABLE", "1")
                .pin("MBT")
                .commit();
        }
        for val in ["CCLK", "USERCLK", "JTAGCLK"] {
            bctx.mode("STARTUP")
                .null_bits()
                .extra_tile_reg_attr(reg, reg_name, "STARTUP", "STARTUPCLK", val)
                .pin("CLK")
                .test_manual("STARTUPCLK", val)
                .global("STARTUPCLK", val)
                .commit();
        }

        let mut bctx = ctx.bel(bels::CAPTURE);
        bctx.test_manual("PRESENT", "1").mode("CAPTURE").commit();
        bctx.mode("CAPTURE")
            .null_bits()
            .extra_tile(IntRelation, "CAPTURE")
            .test_inv("CLK");
        bctx.mode("CAPTURE")
            .null_bits()
            .extra_tile(IntRelation, "CAPTURE")
            .test_inv("CAP");
        if edev.chip.kind.is_spartan3a() {
            for val in ["FALSE", "TRUE"] {
                bctx.mode("CAPTURE")
                    .null_bits()
                    .extra_tile_reg_attr(Reg::Cor2, "REG.COR2.S3A", "CAPTURE", "ONESHOT", val)
                    .test_manual("ONESHOT", val)
                    .attr("ONESHOT", val)
                    .commit();
            }
        } else {
            bctx.mode("CAPTURE")
                .null_bits()
                .extra_tile_reg_attr(Reg::Cor0, reg_cor, "CAPTURE", "ONESHOT_ATTR", "ONE_SHOT")
                .test_manual("ONESHOT_ATTR", "ONE_SHOT")
                .attr("ONESHOT_ATTR", "ONE_SHOT")
                .commit();
        }

        let mut bctx = ctx.bel(bels::ICAP);
        if edev.chip.kind.is_spartan3a() {
            bctx.build()
                .null_bits()
                .extra_tile_reg_attr(Reg::Ctl0, "REG.CTL.S3A", "ICAP", "ENABLE", "1")
                .test_manual("ENABLE", "1")
                .mode("ICAP")
                .commit();
        } else if edev.chip.kind == ChipKind::Spartan3E {
            bctx.build()
                .null_bits()
                .test_manual("ENABLE", "1")
                .mode("ICAP")
                .commit();
        } else {
            bctx.test_manual("ENABLE", "1").mode("ICAP").commit();
        }
        if edev.chip.kind == ChipKind::Spartan3E {
            bctx.mode("ICAP").null_bits().test_inv("CLK");
            bctx.mode("ICAP").null_bits().test_inv("CE");
            bctx.mode("ICAP").null_bits().test_inv("WRITE");
        } else {
            bctx.mode("ICAP")
                .null_bits()
                .extra_tile(IntRelation, "ICAP")
                .test_inv("CLK");
            bctx.mode("ICAP")
                .null_bits()
                .extra_tile(IntRelation, "ICAP")
                .test_inv("CE");
            bctx.mode("ICAP")
                .null_bits()
                .extra_tile(IntRelation, "ICAP")
                .test_inv("WRITE");
        }

        if edev.chip.kind.is_spartan3a() {
            let mut bctx = ctx.bel(bels::SPI_ACCESS);
            bctx.build()
                .extra_tile(IntRelation, "SPI_ACCESS")
                .test_manual("ENABLE", "1")
                .mode("SPI_ACCESS")
                .commit();
        }
    }

    {
        // UR
        let mut ctx = FuzzCtx::new(session, backend, ur);
        if edev.chip.kind != ChipKind::FpgaCore {
            fuzz_pull(&mut ctx, "MISC", "TCKPIN");
            fuzz_pull(&mut ctx, "MISC", "TDOPIN");
            if !edev.chip.kind.is_spartan3a() {
                fuzz_pull(&mut ctx, "MISC", "TMSPIN");
            } else {
                fuzz_pull(&mut ctx, "MISC", "MISO2PIN");
                fuzz_pull(&mut ctx, "MISC", "CSO2PIN");
            }
        }
        let mut bctx = ctx.bel(bels::BSCAN);
        bctx.test_manual("PRESENT", "1").mode("BSCAN").commit();
        bctx.build()
            .test_manual("USERID", "")
            .multi_global("USERID", MultiValue::HexPrefix, 32);
        bctx.mode("BSCAN")
            .no_pin("TDO2")
            .test_manual("TDO1", "1")
            .pin("TDO1")
            .pin_int_pips("TDO1")
            .commit();
        bctx.mode("BSCAN")
            .no_pin("TDO1")
            .test_manual("TDO2", "1")
            .pin("TDO2")
            .pin_int_pips("TDO2")
            .commit();
        if edev.chip.kind.is_virtex2p() {
            let mut bctx = ctx.bel(bels::JTAGPPC);
            bctx.test_manual("PRESENT", "1").mode("JTAGPPC").commit();
        }
    }

    if edev.chip.kind == ChipKind::FpgaCore {
        let mut ctx = FuzzCtx::new_null(session, backend);
        let cnr_ll = edev.chip.corner(DirHV::SW);
        let cnr_ul = edev.chip.corner(DirHV::NW);
        let cnr_lr = edev.chip.corner(DirHV::SE);
        let cnr_ur = edev.chip.corner(DirHV::NE);
        let int_ll = edev.chip.corner(DirHV::SW).cell.tile(tslots::INT);
        let int_ul = edev.chip.corner(DirHV::NW).cell.tile(tslots::INT);
        let int_lr = edev.chip.corner(DirHV::SE).cell.tile(tslots::INT);
        let int_ur = edev.chip.corner(DirHV::NE).cell.tile(tslots::INT);
        for val in ["NO", "YES"] {
            ctx.build()
                .extra_tile_fixed(cnr_ll, "MISC")
                .extra_tile_fixed(cnr_ul, "MISC")
                .extra_tile_fixed(cnr_lr, "MISC")
                .extra_tile_fixed(cnr_ur, "MISC")
                .test_manual("MISC", "MISR_RESET", val)
                .global("MISRRESET", val)
                .commit();
        }
        ctx.build()
            .global_mutex("MISR_CLOCK", "YUP")
            .extra_tile_fixed(cnr_ll, "MISC")
            .extra_tile_fixed(cnr_ul, "MISC")
            .extra_tile_fixed(cnr_lr, "MISC")
            .extra_tile_fixed(cnr_ur, "MISC")
            .extra_tile_fixed(int_ll, "MISC")
            .extra_tile_fixed(int_ul, "MISC")
            .extra_tile_fixed(int_lr, "MISC")
            .extra_tile_fixed(int_ur, "MISC")
            .extra_tiles_by_bel(bels::GCLKVM, "MISC")
            .extra_tiles_by_bel(bels::GCLKH, "MISC")
            .test_manual("MISC", "MISR_CLOCK", "GCLK0")
            .global("MISRCLOCK", "GCLK0")
            .commit();
    }

    // I/O bank misc control
    if !skip_io && edev.chip.kind != ChipKind::FpgaCore {
        let package = backend
            .device
            .bonds
            .values()
            .max_by_key(|bond| {
                let bdata = &backend.db.bonds[bond.bond];
                let prjcombine_re_xilinx_geom::Bond::Virtex2(bdata) = bdata else {
                    unreachable!();
                };
                bdata.pins.len()
            })
            .unwrap();
        let ExpandedBond::Virtex2(ref ebond) = backend.ebonds[&package.name] else {
            unreachable!()
        };
        if !edev.chip.kind.is_spartan3ea() {
            for (dir, tile_name, bel, bank) in [
                (DirHV::NW, ul, 0, 7),
                (DirHV::NW, ul, 1, 0),
                (DirHV::NE, ur, 1, 1),
                (DirHV::NE, ur, 0, 2),
                (DirHV::SE, lr, 0, 3),
                (DirHV::SE, lr, 1, 4),
                (DirHV::SW, ll, 1, 5),
                (DirHV::SW, ll, 0, 6),
            ] {
                let mut ctx = FuzzCtx::new(session, backend, tile_name);
                let mut bctx = ctx.bel([bels::DCI0, bels::DCI1][bel]);

                let bel_name = ["DCI0", "DCI1"][bel];
                let mut btiles = vec![edev.btile_lrterm(edev.chip.corner(dir).cell)];
                if edev.chip.kind.is_virtex2() {
                    btiles.push(edev.btile_btterm(edev.chip.corner(dir).cell));
                }
                let mut site = None;
                let mut site_other = None;
                let mut coords = HashSet::new();
                let other_bank = if bank == 4 { 5 } else { 4 };
                let mut io_vr = None;
                if let Some(&(vrp, vrn)) = edev.chip.dci_io.get(&bank)
                    && ebond.ios.contains_key(&vrp)
                    && ebond.ios.contains_key(&vrn)
                {
                    io_vr = Some((vrp, vrn));
                }
                if io_vr.is_none() {
                    io_vr = Some(edev.chip.dci_io_alt[&bank]);
                }
                let (io_vrp, io_vrn) = io_vr.unwrap();
                let site_vrp = endev.get_io_name(io_vrp);
                let site_vrn = endev.get_io_name(io_vrn);
                for io in edev.chip.get_bonded_ios().into_iter().rev() {
                    let ioinfo = edev.chip.get_io_info(io);
                    let bcrd = edev.chip.get_io_loc(io);
                    if ioinfo.bank == bank && coords.insert(bcrd.cell) {
                        btiles.push(edev.btile_main(bcrd.cell));
                        if bcrd.cell.col == edev.chip.col_w() || bcrd.cell.col == edev.chip.col_e()
                        {
                            btiles.push(edev.btile_lrterm(bcrd.cell));
                        } else {
                            btiles.push(edev.btile_btterm(bcrd.cell));
                        }
                    }
                    if ebond.ios.contains_key(&io)
                        && matches!(ioinfo.diff, IoDiffKind::P(_))
                        && ioinfo.pad_kind == Some(IobKind::Iob)
                        && io != io_vrp
                        && io != io_vrn
                    {
                        if ioinfo.bank == bank && site.is_none() {
                            site = Some(endev.get_io_name(io));
                        }
                        if ioinfo.bank == other_bank && site_other.is_none() {
                            site_other = Some(endev.get_io_name(io));
                        }
                    }
                }
                let site = site.unwrap();
                let site_other = site_other.unwrap();
                for std in get_iostds(edev, false) {
                    if std.diff == DiffKind::True {
                        bctx.build()
                            .prop(ForceBits(btiles.clone()))
                            .raw(Key::Package, package.name.clone())
                            .global_mutex("DIFF", "BANK")
                            .global_mutex("VREF", "NO")
                            .global_mutex("DCI", "YES")
                            .test_manual("LVDSBIAS", std.name)
                            .raw_diff(Key::SiteMode(site), None, "DIFFM")
                            .raw_diff(Key::SiteAttr(site, "OMUX".into()), None, "O1")
                            .raw_diff(Key::SiteAttr(site, "O1INV".into()), None, "O1")
                            .raw_diff(Key::SiteAttr(site, "IOATTRBOX".into()), None, std.name)
                            .raw_diff(Key::SitePin(site, "O1".into()), None, true)
                            .commit();
                    }
                    if matches!(
                        std.dci,
                        DciKind::InputSplit | DciKind::BiSplit | DciKind::InputVcc | DciKind::BiVcc
                    ) && std.diff == DiffKind::None
                    {
                        bctx.build()
                            .prop(ForceBits(btiles.clone()))
                            .raw(Key::Package, package.name.clone())
                            .global_mutex("VREF", "NO")
                            .global_mutex("DCI", "BANK_TERM")
                            .raw(Key::SiteMode(site_other), "IOB")
                            .raw(Key::SiteAttr(site_other, "OMUX".into()), "O1")
                            .raw(Key::SiteAttr(site_other, "O1INV".into()), "O1")
                            .raw(Key::SiteAttr(site_other, "IOATTRBOX".into()), "LVDCI_33")
                            .raw(Key::SitePin(site_other, "O1".into()), true)
                            .raw(Key::SiteMode(site_vrp), None)
                            .raw(Key::SiteMode(site_vrn), None)
                            .raw(Key::SiteAttr(site, "IMUX".into()), "1")
                            .raw(Key::SitePin(site, "I".into()), true)
                            .test_manual("DCI_TERM", std.name)
                            .raw_diff(Key::SiteMode(site), "IOB", "IOB")
                            .raw_diff(Key::SiteAttr(site, "IOATTRBOX".into()), "GTL", std.name)
                            .commit();
                    }
                }
                if edev.chip.kind == ChipKind::Spartan3 {
                    for val in ["ASREQUIRED", "CONTINUOUS", "QUIET"] {
                        bctx.build()
                            .prop(ForceBits(btiles.clone()))
                            .raw(Key::Package, package.name.clone())
                            .global_mutex("VREF", "NO")
                            .global_mutex("DCI", "BANK")
                            .raw(Key::SiteMode(site_other), "IOB")
                            .raw(Key::SiteAttr(site_other, "OMUX".into()), "O1")
                            .raw(Key::SiteAttr(site_other, "O1INV".into()), "O1")
                            .raw(Key::SiteAttr(site_other, "IOATTRBOX".into()), "LVDCI_33")
                            .raw(Key::SitePin(site_other, "O1".into()), true)
                            .raw(Key::SiteMode(site_vrp), None)
                            .raw(Key::SiteMode(site_vrn), None)
                            .global("DCIUPDATEMODE", val)
                            .test_manual("DCI_OUT", val)
                            .raw_diff(Key::SiteMode(site), None, "IOB")
                            .raw_diff(Key::SiteAttr(site, "OMUX".into()), None, "O1")
                            .raw_diff(Key::SiteAttr(site, "O1INV".into()), None, "O1")
                            .raw_diff(Key::SiteAttr(site, "IOATTRBOX".into()), None, "LVDCI_33")
                            .raw_diff(Key::SitePin(site, "O1".into()), None, true)
                            .commit();
                    }
                } else {
                    bctx.build()
                        .prop(ForceBits(btiles.clone()))
                        .raw(Key::Package, package.name.clone())
                        .global_mutex("VREF", "NO")
                        .global_mutex("DCI", "BANK")
                        .raw(Key::SiteMode(site_other), "IOB")
                        .raw(Key::SiteAttr(site_other, "OMUX".into()), "O1")
                        .raw(Key::SiteAttr(site_other, "O1INV".into()), "O1")
                        .raw(Key::SiteAttr(site_other, "IOATTRBOX".into()), "LVDCI_33")
                        .raw(Key::SitePin(site_other, "O1".into()), true)
                        .raw(Key::SiteMode(site_vrp), None)
                        .raw(Key::SiteMode(site_vrn), None)
                        .test_manual("DCI_OUT", "1")
                        .raw_diff(Key::SiteMode(site), None, "IOB")
                        .raw_diff(Key::SiteAttr(site, "OMUX".into()), None, "O1")
                        .raw_diff(Key::SiteAttr(site, "O1INV".into()), None, "O1")
                        .raw_diff(Key::SiteAttr(site, "IOATTRBOX".into()), None, "LVDCI_33")
                        .raw_diff(Key::SitePin(site, "O1".into()), None, true)
                        .commit();
                }
                if bank == 6 {
                    let mut builder = bctx
                        .build()
                        .prop(ForceBits(btiles.clone()))
                        .raw(Key::Package, package.name.clone())
                        .global_mutex("VREF", "NO")
                        .global_mutex("DCI", "GLOBAL")
                        .global("MATCH_CYCLE", "NOWAIT")
                        .raw(Key::SiteMode(site_vrp), None)
                        .raw(Key::SiteMode(site_vrn), None);
                    if edev.chip.kind != ChipKind::Spartan3 {
                        builder = builder.global("FREEZEDCI", "NO");
                    }
                    builder
                        .test_manual("DCI_OUT_ALONE", "1")
                        .raw_diff(Key::SiteMode(site), None, "IOB")
                        .raw_diff(Key::SiteAttr(site, "OMUX".into()), None, "O1")
                        .raw_diff(Key::SiteAttr(site, "O1INV".into()), None, "O1")
                        .raw_diff(Key::SiteAttr(site, "IOATTRBOX".into()), None, "LVDCI_33")
                        .raw_diff(Key::SitePin(site, "O1".into()), None, true)
                        .commit();
                } else if bank == 5 && edev.chip.dci_io_alt.contains_key(&5) {
                    let (io_alt_vrp, io_alt_vrn) = edev.chip.dci_io_alt[&5];
                    let site_alt_vrp = endev.get_io_name(io_alt_vrp);
                    let site_alt_vrn = endev.get_io_name(io_alt_vrn);
                    let mut builder = bctx
                        .build()
                        .prop(ForceBits(btiles.clone()))
                        .raw(Key::Package, package.name.clone())
                        .raw(Key::AltVr, true)
                        .global_mutex("VREF", "NO")
                        .global_mutex("DCI", "GLOBAL_ALT")
                        .global("MATCH_CYCLE", "NOWAIT");
                    if site != site_alt_vrp {
                        builder = builder.raw(Key::SiteMode(site_alt_vrp), None);
                    }
                    if site != site_alt_vrn {
                        builder = builder.raw(Key::SiteMode(site_alt_vrn), None);
                    }
                    builder
                        .test_manual("DCI_OUT_ALONE", "1")
                        .raw_diff(Key::SiteMode(site), None, "IOB")
                        .raw_diff(Key::SiteAttr(site, "OMUX".into()), None, "O1")
                        .raw_diff(Key::SiteAttr(site, "O1INV".into()), None, "O1")
                        .raw_diff(Key::SiteAttr(site, "IOATTRBOX".into()), None, "LVDCI_33")
                        .raw_diff(Key::SitePin(site, "O1".into()), None, true)
                        .commit();
                }
                if edev.chip.kind == ChipKind::Spartan3 {
                    bctx.build()
                        .global_mutex("DCI", "PRESENT")
                        .test_manual("PRESENT", "1")
                        .mode("DCI")
                        .commit();
                    bctx.build()
                        .global_mutex("DCI", "PRESENT")
                        .global_mutex("DCI_SELECT", bel_name)
                        .mode("DCI")
                        .test_manual("SELECT", "1")
                        .pip((PinFar, "DATA"), "DATA")
                        .commit();
                    for i in 0..13 {
                        let name = format!("LVDSBIAS_OPT{i}");
                        let gname = format!("LVDSBIAS_OPT{i}_{bank}");
                        bctx.build()
                            .global_mutex("DIFF", "MANUAL")
                            .test_manual(name, "1")
                            .global_diff(gname, "0", "1")
                            .commit();
                    }
                } else {
                    bctx.build()
                        .global_mutex("DCI", "PRESENT")
                        .test_manual("PRESENT", "1")
                        .mode("DCI")
                        .commit();
                    bctx.build()
                        .global_mutex("DCI", "PRESENT_TEST")
                        .global("TESTDCI", "YES")
                        .test_manual("PRESENT", "TEST")
                        .mode("DCI")
                        .commit();
                }
                // ???
                bctx.mode("DCI")
                    .global_mutex("DCI", "PRESENT")
                    .test_manual("FORCE_DONE_HIGH", "#OFF")
                    .attr("FORCE_DONE_HIGH", "#OFF")
                    .commit();
            }

            if edev.chip.kind.is_virtex2p()
                && !backend.device.name.ends_with("2vp4")
                && !backend.device.name.ends_with("2vp7")
            {
                let mut ctx = FuzzCtx::new(session, backend, ll);
                let btiles = vec![
                    edev.btile_btterm(edev.chip.corner(DirHV::NW).cell),
                    edev.btile_btterm(edev.chip.corner(DirHV::NE).cell),
                    edev.btile_lrterm(edev.chip.corner(DirHV::NE).cell),
                    edev.btile_lrterm(edev.chip.corner(DirHV::SE).cell),
                    edev.btile_btterm(edev.chip.corner(DirHV::SE).cell),
                    edev.btile_btterm(edev.chip.corner(DirHV::SW).cell),
                    edev.btile_lrterm(edev.chip.corner(DirHV::SW).cell),
                    edev.btile_lrterm(edev.chip.corner(DirHV::NW).cell),
                ];
                for val in ["ASREQUIRED", "CONTINUOUS", "QUIET"] {
                    ctx.build()
                        .global_mutex("DCI", "GLOBAL_MODE")
                        .prop(ForceBits(btiles.clone()))
                        .test_manual("MISC", "DCIUPDATEMODE", val)
                        .global("DCIUPDATEMODE", val)
                        .commit();
                }
            }
        } else {
            let banks = if edev.chip.kind == ChipKind::Spartan3E {
                &[
                    (ul, edev.btile_lrterm(edev.chip.corner(DirHV::NW).cell), 0),
                    (ur, edev.btile_lrterm(edev.chip.corner(DirHV::NE).cell), 1),
                    (lr, edev.btile_lrterm(edev.chip.corner(DirHV::SE).cell), 2),
                    (ll, edev.btile_lrterm(edev.chip.corner(DirHV::SW).cell), 3),
                ][..]
            } else {
                &[
                    (ul, edev.btile_lrterm(edev.chip.corner(DirHV::NW).cell), 0),
                    (ll, edev.btile_lrterm(edev.chip.corner(DirHV::SW).cell), 2),
                ][..]
            };
            for &(tile_name, btile, bank) in banks {
                let mut ctx = FuzzCtx::new(session, backend, tile_name);
                let mut btiles = vec![btile];
                match bank {
                    0 => {
                        let row = edev.chip.row_n();
                        for col in edev.chip.columns.ids() {
                            if col != edev.chip.col_w() && col != edev.chip.col_e() {
                                let cell = CellCoord::new(DieId::from_idx(0), col, row);
                                btiles.push(edev.btile_main(cell));
                                btiles.push(edev.btile_btterm(cell));
                            }
                        }
                    }
                    1 => {
                        let col = edev.chip.col_e();
                        for row in edev.chip.rows.ids() {
                            if row != edev.chip.row_s() && row != edev.chip.row_n() {
                                let cell = CellCoord::new(DieId::from_idx(0), col, row);
                                btiles.push(edev.btile_main(cell));
                                btiles.push(edev.btile_lrterm(cell));
                            }
                        }
                    }
                    2 => {
                        let row = edev.chip.row_s();
                        for col in edev.chip.columns.ids() {
                            if col != edev.chip.col_w() && col != edev.chip.col_e() {
                                let cell = CellCoord::new(DieId::from_idx(0), col, row);
                                btiles.push(edev.btile_main(cell));
                                btiles.push(edev.btile_btterm(cell));
                            }
                        }
                    }
                    3 => {
                        let col = edev.chip.col_w();
                        for row in edev.chip.rows.ids() {
                            if row != edev.chip.row_s() && row != edev.chip.row_n() {
                                let cell = CellCoord::new(DieId::from_idx(0), col, row);
                                btiles.push(edev.btile_main(cell));
                                btiles.push(edev.btile_lrterm(cell));
                            }
                        }
                    }
                    _ => unreachable!(),
                }
                let mut ios = vec![];
                for io in edev.chip.get_bonded_ios().into_iter().rev() {
                    let ioinfo = edev.chip.get_io_info(io);
                    if ebond.ios.contains_key(&io)
                        && matches!(ioinfo.diff, IoDiffKind::P(_))
                        && ioinfo.pad_kind == Some(IobKind::Iob)
                        && ioinfo.bank == bank
                    {
                        ios.push(io)
                    }
                }
                assert!(ios.len() >= 2);
                if edev.chip.kind == ChipKind::Spartan3ADsp {
                    ios.reverse();
                }
                let site_a = endev.get_io_name(ios[0]);
                let site_b = endev.get_io_name(ios[1]);
                let diffm = if edev.chip.kind == ChipKind::Spartan3E {
                    "DIFFM"
                } else {
                    "DIFFMTB"
                };
                for std in get_iostds(edev, false) {
                    if std.diff != DiffKind::True {
                        continue;
                    }
                    if std.name != "LVDS_25" || edev.chip.kind.is_spartan3a() {
                        ctx.build()
                            .prop(ForceBits(btiles.clone()))
                            .raw(Key::Package, package.name.clone())
                            .global_mutex("DIFF", "BANK")
                            .global_mutex("VREF", "NO")
                            .test_manual("BANK", "LVDSBIAS_0", std.name)
                            .raw_diff(Key::SiteMode(site_a), None, diffm)
                            .raw_diff(Key::SiteAttr(site_a, "OMUX".into()), None, "O1")
                            .raw_diff(Key::SiteAttr(site_a, "O1INV".into()), None, "O1")
                            .raw_diff(Key::SiteAttr(site_a, "IOATTRBOX".into()), None, std.name)
                            .raw_diff(
                                Key::SiteAttr(site_a, "SUSPEND".into()),
                                None,
                                if edev.chip.kind.is_spartan3a() {
                                    "3STATE"
                                } else {
                                    ""
                                },
                            )
                            .raw_diff(Key::SitePin(site_a, "O1".into()), None, true)
                            .commit();
                    }
                    let alt_std = if std.name == "RSDS_25" {
                        "MINI_LVDS_25"
                    } else {
                        "RSDS_25"
                    };
                    ctx.build()
                        .prop(ForceBits(btiles.clone()))
                        .raw(Key::Package, package.name.clone())
                        .global_mutex("DIFF", "BANK")
                        .raw(Key::SiteMode(site_a), diffm)
                        .raw(Key::SiteAttr(site_a, "OMUX".into()), "O1")
                        .raw(Key::SiteAttr(site_a, "O1INV".into()), "O1")
                        .raw(Key::SiteAttr(site_a, "IOATTRBOX".into()), alt_std)
                        .raw(
                            Key::SiteAttr(site_a, "SUSPEND".into()),
                            if edev.chip.kind.is_spartan3a() {
                                "3STATE"
                            } else {
                                ""
                            },
                        )
                        .raw(Key::SitePin(site_a, "O1".into()), true)
                        .test_manual("BANK", "LVDSBIAS_1", std.name)
                        .raw_diff(Key::SiteMode(site_b), None, diffm)
                        .raw_diff(Key::SiteAttr(site_b, "OMUX".into()), None, "O1")
                        .raw_diff(Key::SiteAttr(site_b, "O1INV".into()), None, "O1")
                        .raw_diff(Key::SiteAttr(site_b, "IOATTRBOX".into()), None, std.name)
                        .raw_diff(
                            Key::SiteAttr(site_b, "SUSPEND".into()),
                            None,
                            if edev.chip.kind.is_spartan3a() {
                                "3STATE"
                            } else {
                                ""
                            },
                        )
                        .raw_diff(Key::SitePin(site_b, "O1".into()), None, true)
                        .commit();
                }
            }
        }
    }

    // config regs
    if !edev.chip.kind.is_spartan3a() {
        let mut ctx = FuzzCtx::new_null(session, backend);
        {
            let reg = Reg::Cor0;
            let reg_name = reg_cor;
            for val in ["1", "2", "3", "4", "5", "6", "DONE", "KEEP"] {
                ctx.test_reg(reg, reg_name, "STARTUP", "GWE_CYCLE", val)
                    .global("GWE_CYCLE", val)
                    .commit();
                ctx.test_reg(reg, reg_name, "STARTUP", "GTS_CYCLE", val)
                    .global("GTS_CYCLE", val)
                    .commit();
            }
            for val in ["1", "2", "3", "4", "5", "6", "KEEP"] {
                ctx.test_reg(reg, reg_name, "STARTUP", "DONE_CYCLE", val)
                    .global("DONE_CYCLE", val)
                    .commit();
            }
            for val in ["0", "1", "2", "3", "4", "5", "6", "NOWAIT"] {
                if edev.chip.kind != ChipKind::FpgaCore {
                    ctx.test_reg(reg, reg_name, "STARTUP", "LCK_CYCLE", val)
                        .global("LCK_CYCLE", val)
                        .commit();
                }
                if edev.chip.kind != ChipKind::Spartan3E && edev.chip.kind != ChipKind::FpgaCore {
                    // option is accepted on S3E, but doesn't do anything
                    ctx.build()
                        .global_mutex("DCI", "NO")
                        .test_reg(reg, reg_name, "STARTUP", "MATCH_CYCLE", val)
                        .global("MATCH_CYCLE", val)
                        .commit();
                }
            }
            for val in ["NO", "YES"] {
                ctx.test_reg(reg, reg_name, "STARTUP", "DRIVE_DONE", val)
                    .global("DRIVEDONE", val)
                    .commit();
                ctx.test_reg(reg, reg_name, "STARTUP", "DONE_PIPE", val)
                    .global("DONEPIPE", val)
                    .commit();
            }
            for val in ["ENABLE", "DISABLE"] {
                if edev.chip.kind != ChipKind::FpgaCore {
                    ctx.test_reg(reg, reg_name, "STARTUP", "DCM_SHUTDOWN", val)
                        .global("DCMSHUTDOWN", val)
                        .commit();
                }
                if edev.chip.kind.is_virtex2() {
                    ctx.test_reg(reg, reg_name, "STARTUP", "DCI_SHUTDOWN", val)
                        .global("DCISHUTDOWN", val)
                        .commit();
                    ctx.test_reg(reg, reg_name, "STARTUP", "POWERDOWN_STATUS", val)
                        .global("POWERDOWNSTATUS", val)
                        .commit();
                }
            }
            let vals = if edev.chip.kind.is_virtex2() {
                &[
                    "4", "5", "7", "8", "9", "10", "13", "15", "20", "26", "30", "34", "41", "51",
                    "55", "60", "130",
                ][..]
            } else if !edev.chip.kind.is_spartan3ea() {
                &["6", "12", "25", "50", "3", "100"][..]
            } else {
                &["1", "3", "6", "12", "25", "50"][..]
            };
            for &val in vals {
                ctx.test_reg(reg, reg_name, "STARTUP", "CONFIG_RATE", val)
                    .global("CONFIGRATE", val)
                    .commit();
            }
            for val in ["DISABLE", "ENABLE"] {
                ctx.test_reg(reg, reg_name, "STARTUP", "CRC", val)
                    .global("CRC", val)
                    .commit();
            }
            if !edev.chip.kind.is_virtex2() {
                for val in ["100", "25", "50", "200"] {
                    ctx.test_reg(reg, reg_name, "STARTUP", "BUSCLK_FREQ", val)
                        .global("BUSCLKFREQ", val)
                        .commit();
                }
                let vals = if !edev.chip.kind.is_spartan3ea() {
                    &["80", "90", "95", "100"]
                } else {
                    &["70", "75", "80", "90"]
                };
                for &val in vals {
                    ctx.test_reg(reg, reg_name, "STARTUP", "VRDSEL", val)
                        .global("VRDSEL", val)
                        .commit();
                }
            }
        }

        {
            let reg_name = if edev.chip.kind.is_virtex2() {
                "REG.CTL"
            } else {
                "REG.CTL.S3"
            };
            let reg = Reg::Ctl0;
            for val in ["NO", "YES"] {
                ctx.test_reg(reg, reg_name, "MISC", "GTS_USR_B", val)
                    .global("GTS_USR_B", val)
                    .commit();
                ctx.test_reg(reg, reg_name, "MISC", "VGG_TEST", val)
                    .global("VGG_TEST", val)
                    .commit();
                ctx.test_reg(reg, reg_name, "MISC", "BCLK_TEST", val)
                    .global("BCLK_TEST", val)
                    .commit();
            }
            // persist not fuzzed — too much effort
            for val in ["NONE", "LEVEL1", "LEVEL2"] {
                // disables FreezeDCI?
                if edev.chip.kind == ChipKind::Virtex2 {
                    ctx.build()
                        .global_mutex("DCI", "NO")
                        .global("EARLYGHIGH", "YES")
                        .test_reg(reg, reg_name, "MISC", "SECURITY", val)
                        .global("SECURITY", val)
                        .commit();
                } else {
                    ctx.build()
                        .global_mutex("DCI", "NO")
                        .test_reg(reg, reg_name, "MISC", "SECURITY", val)
                        .global("SECURITY", val)
                        .commit();
                }
            }

            if edev.chip.kind.is_virtex2() {
                ctx.build()
                    .global_mutex("DCI", "NO")
                    .test_manual("MISC", "ENCRYPT", "YES")
                    .global("ENCRYPT", "YES")
                    .commit();
            }
        }
    } else {
        let mut ctx = FuzzCtx::new_null(session, backend);
        {
            let reg = Reg::Cor1;
            let reg_name = "REG.COR1.S3A";
            for val in ["NO", "YES"] {
                ctx.test_reg(reg, reg_name, "STARTUP", "DRIVE_DONE", val)
                    .global("DRIVEDONE", val)
                    .commit();
                ctx.test_reg(reg, reg_name, "STARTUP", "DONE_PIPE", val)
                    .global("DONEPIPE", val)
                    .commit();
                ctx.test_reg(reg, reg_name, "STARTUP", "DRIVE_AWAKE", val)
                    .global("DRIVE_AWAKE", val)
                    .commit();
            }
            for val in ["DISABLE", "ENABLE"] {
                ctx.test_reg(reg, reg_name, "STARTUP", "CRC", val)
                    .global("CRC", val)
                    .commit();
            }
            ctx.test_reg(reg, reg_name, "STARTUP", "VRDSEL", "")
                .multi_global("VRDSEL", MultiValue::Bin, 3);
        }

        {
            let reg = Reg::Cor2;
            let reg_name = "REG.COR2.S3A";
            for val in ["1", "2", "3", "4", "5", "6", "DONE", "KEEP"] {
                ctx.test_reg(reg, reg_name, "STARTUP", "GWE_CYCLE", val)
                    .global("GWE_CYCLE", val)
                    .commit();
                ctx.test_reg(reg, reg_name, "STARTUP", "GTS_CYCLE", val)
                    .global("GTS_CYCLE", val)
                    .commit();
            }
            for val in ["1", "2", "3", "4", "5", "6"] {
                ctx.test_reg(reg, reg_name, "STARTUP", "DONE_CYCLE", val)
                    .global("DONE_CYCLE", val)
                    .commit();
            }
            for val in ["1", "2", "3", "4", "5", "6", "NOWAIT"] {
                ctx.test_reg(reg, reg_name, "STARTUP", "LCK_CYCLE", val)
                    .global("LCK_CYCLE", val)
                    .commit();
            }
            for val in ["NO", "YES"] {
                ctx.test_reg(reg, reg_name, "STARTUP", "BPI_DIV8", val)
                    .global("BPI_DIV8", val)
                    .commit();
                ctx.test_reg(reg, reg_name, "STARTUP", "RESET_ON_ERR", val)
                    .global("RESET_ON_ERR", val)
                    .commit();
            }
            for val in ["NO", "YES"] {
                ctx.test_reg(reg, reg_name, "ICAP", "BYPASS", val)
                    .global("ICAP_BYPASS", val)
                    .commit();
            }
        }

        {
            let reg = Reg::Ctl0;
            let reg_name = "REG.CTL.S3A";
            for val in ["NO", "YES"] {
                ctx.test_reg(reg, reg_name, "MISC", "GTS_USR_B", val)
                    .global("GTS_USR_B", val)
                    .commit();
                ctx.test_reg(reg, reg_name, "MISC", "VGG_TEST", val)
                    .global("VGG_TEST", val)
                    .commit();
                ctx.test_reg(reg, reg_name, "MISC", "MULTIBOOT_ENABLE", val)
                    .global("MULTIBOOTMODE", val)
                    .commit();
            }
            // persist not fuzzed — too much effort
            for val in ["NONE", "LEVEL1", "LEVEL2", "LEVEL3"] {
                ctx.test_reg(reg, reg_name, "MISC", "SECURITY", val)
                    .global("SECURITY", val)
                    .commit();
            }
        }

        {
            let reg = Reg::CclkFrequency;
            let reg_name = "REG.CCLK_FREQ";
            for val in [
                "6", "1", "3", "7", "8", "10", "12", "13", "17", "22", "25", "27", "33", "44",
                "50", "100",
            ] {
                ctx.test_reg(reg, reg_name, "STARTUP", "CONFIG_RATE", val)
                    .global("CONFIGRATE", val)
                    .commit();
            }
            for val in ["0", "1", "2", "3"] {
                ctx.test_reg(reg, reg_name, "STARTUP", "CCLK_DLY", val)
                    .global("CCLK_DLY", val)
                    .commit();
                ctx.test_reg(reg, reg_name, "STARTUP", "CCLK_SEP", val)
                    .global("CCLK_SEP", val)
                    .commit();
                ctx.test_reg(reg, reg_name, "STARTUP", "CLK_SWITCH_OPT", val)
                    .global("CLK_SWITCH_OPT", val)
                    .commit();
            }
        }

        {
            let reg = Reg::HcOpt;
            let reg_name = "REG.HC_OPT";
            for val in ["NO", "YES"] {
                ctx.test_reg(reg, reg_name, "MISC", "BRAM_SKIP", val)
                    .global("BRAM_SKIP", val)
                    .commit();
                ctx.test_reg(reg, reg_name, "MISC", "TWO_ROUND", val)
                    .global("TWO_ROUND", val)
                    .commit();
            }
            for i in 1..16 {
                let val = format!("{i}");
                ctx.test_reg(reg, reg_name, "MISC", "HC_CYCLE", &val)
                    .global("HC_CYCLE", &val)
                    .commit();
            }
        }

        {
            let reg = Reg::Powerdown;
            let reg_name = "REG.POWERDOWN";
            for val in ["STARTUPCLK", "INTERNALCLK"] {
                ctx.test_reg(reg, reg_name, "MISC", "SW_CLK", val)
                    .global("SW_CLK", val)
                    .commit();
            }
            for val in ["NO", "YES"] {
                ctx.test_reg(reg, reg_name, "MISC", "EN_SUSPEND", val)
                    .global("EN_SUSPEND", val)
                    .commit();
                ctx.test_reg(reg, reg_name, "MISC", "EN_PORB", val)
                    .global("EN_PORB", val)
                    .commit();
                ctx.test_reg(reg, reg_name, "MISC", "SUSPEND_FILTER", val)
                    .global("SUSPEND_FILTER", val)
                    .commit();
                ctx.test_reg(reg, reg_name, "MISC", "EN_SW_GSR", val)
                    .global("EN_SW_GSR", val)
                    .commit();
            }
            for i in 1..8 {
                let val = format!("{i}");
                ctx.test_reg(reg, reg_name, "MISC", "WAKE_DELAY1", &val)
                    .global("WAKE_DELAY1", val)
                    .commit();
            }
            for i in 1..32 {
                let val = format!("{i}");
                ctx.test_reg(reg, reg_name, "MISC", "WAKE_DELAY2", &val)
                    .global("WAKE_DELAY2", val)
                    .commit();
            }
        }

        {
            let reg = Reg::PuGwe;
            let reg_name = "REG.PU_GWE";
            ctx.test_reg(reg, reg_name, "MISC", "SW_GWE_CYCLE", "")
                .multi_global("SW_GWE_CYCLE", MultiValue::Dec(0), 10);
        }

        {
            let reg = Reg::PuGts;
            let reg_name = "REG.PU_GTS";
            ctx.test_reg(reg, reg_name, "MISC", "SW_GTS_CYCLE", "")
                .multi_global("SW_GTS_CYCLE", MultiValue::Dec(0), 10);
        }

        {
            let reg = Reg::Mode;
            let reg_name = "REG.MODE";
            for val in ["NO", "YES"] {
                ctx.test_reg(reg, reg_name, "MISC", "TESTMODE_EN", val)
                    .global("TESTMODE_EN", val)
                    .commit();
                ctx.test_reg(reg, reg_name, "MISC", "NEXT_CONFIG_NEW_MODE", val)
                    .global("NEXT_CONFIG_NEW_MODE", val)
                    .commit();
            }
            ctx.test_reg(reg, reg_name, "MISC", "NEXT_CONFIG_BOOT_MODE", "")
                .multi_global("NEXT_CONFIG_BOOT_MODE", MultiValue::Bin, 3);
            ctx.test_reg(reg, reg_name, "MISC", "BOOTVSEL", "")
                .multi_global("BOOTVSEL", MultiValue::Bin, 3);
        }

        {
            ctx.build()
                .prop(ExtraReg::new(
                    vec![Reg::General1, Reg::General2],
                    false,
                    "REG.GENERAL".into(),
                    Some("MISC".into()),
                    Some("NEXT_CONFIG_ADDR".into()),
                    Some("".into()),
                ))
                .test_manual("MISC", "NEXT_CONFIG_ADDR", "")
                .multi_global("NEXT_CONFIG_ADDR", MultiValue::HexPrefix, 32);
        }

        {
            let reg = Reg::SeuOpt;
            let reg_name = "REG.SEU_OPT";
            for val in ["NO", "YES"] {
                ctx.test_reg(reg, reg_name, "MISC", "GLUTMASK", val)
                    .global("GLUTMASK", val)
                    .commit();
                ctx.test_reg(reg, reg_name, "MISC", "POST_CRC_KEEP", val)
                    .global("POST_CRC_KEEP", val)
                    .commit();
            }
            for val in [
                "6", "1", "3", "7", "8", "10", "12", "13", "17", "22", "25", "27", "33", "44",
                "50", "100",
            ] {
                ctx.test_reg(reg, reg_name, "MISC", "POST_CRC_FREQ", val)
                    .global("POST_CRC_FREQ", val)
                    .commit();
            }
        }

        for val in ["NO", "YES"] {
            ctx.test_manual("NULL", "SPI2_EN", val)
                .global("SPI2_EN", val)
                .commit();
            ctx.test_manual("NULL", "BRAMMASK", val)
                .global("BRAMMASK", val)
                .commit();
        }

        // TODO
    }
}

pub fn collect_fuzzers(ctx: &mut CollectorCtx, skip_io: bool, devdata_only: bool) {
    let ExpandedDevice::Virtex2(edev) = ctx.edev else {
        unreachable!()
    };
    let int_tiles = if edev.chip.kind.is_virtex2() {
        &["INT.CNR"]
    } else {
        &["INT.CLB"]
    };

    let (ll, ul, lr, ur) = match edev.chip.kind {
        ChipKind::Virtex2 => ("LL.V2", "UL.V2", "LR.V2", "UR.V2"),
        ChipKind::Virtex2P | ChipKind::Virtex2PX => ("LL.V2P", "UL.V2P", "LR.V2P", "UR.V2P"),
        ChipKind::Spartan3 => ("LL.S3", "UL.S3", "LR.S3", "UR.S3"),
        ChipKind::FpgaCore => ("LL.FC", "UL.FC", "LR.FC", "UR.FC"),
        ChipKind::Spartan3E => ("LL.S3E", "UL.S3E", "LR.S3E", "UR.S3E"),
        ChipKind::Spartan3A | ChipKind::Spartan3ADsp => ("LL.S3A", "UL.S3A", "LR.S3A", "UR.S3A"),
    };

    if devdata_only {
        let tile = ll;
        let bel = "MISC";
        if !edev.chip.kind.is_virtex2() {
            let sendmax = ctx.collect_enum_bool_default(tile, bel, "VGG_SENDMAX", "NO", "YES");
            ctx.insert_device_data("MISC:VGG_SENDMAX_DEFAULT", [sendmax]);
            let (_, vgg0) = ctx.extract_enum_bool_default(tile, bel, "SEND_VGG0", "0", "1");
            let (_, vgg1) = ctx.extract_enum_bool_default(tile, bel, "SEND_VGG1", "0", "1");
            let (_, vgg2) = ctx.extract_enum_bool_default(tile, bel, "SEND_VGG2", "0", "1");
            let (_, vgg3) = ctx.extract_enum_bool_default(tile, bel, "SEND_VGG3", "0", "1");
            ctx.insert_device_data("MISC:SEND_VGG_DEFAULT", [vgg0, vgg1, vgg2, vgg3]);
            if edev.chip.kind.is_spartan3a() {
                let tile = "REG.COR1.S3A";
                let sendmax = ctx.collect_enum_bool_default(tile, bel, "VGG_SENDMAX", "NO", "YES");
                ctx.insert_device_data("MISC:VGG_SENDMAX_DEFAULT", [sendmax]);
                let (_, vgg0) = ctx.extract_enum_bool_default(tile, bel, "SEND_VGG0", "0", "1");
                let (_, vgg1) = ctx.extract_enum_bool_default(tile, bel, "SEND_VGG1", "0", "1");
                let (_, vgg2) = ctx.extract_enum_bool_default(tile, bel, "SEND_VGG2", "0", "1");
                let (_, vgg3) = ctx.extract_enum_bool_default(tile, bel, "SEND_VGG3", "0", "1");
                ctx.insert_device_data("MISC:SEND_VGG_DEFAULT", [vgg0, vgg1, vgg2, vgg3]);
            }
        }
        if edev.chip.kind.is_virtex2() {
            let diff = ctx.state.get_diff(tile, bel, "FREEZE_DCI", "1");
            let diff = diff.filter_tiles(&[4]);
            let mut freeze_dci_nops = 0;
            for (bit, val) in diff.bits {
                assert!(val);
                freeze_dci_nops |= 1 << bit.bit;
            }
            ctx.insert_device_data("FREEZE_DCI_NOPS", freeze_dci_nops);

            let is_double_grestore = ctx.empty_bs.die[DieId::from_idx(0)]
                .regs
                .get(&Reg::FakeDoubleGrestore)
                == Some(&1);
            ctx.insert_device_data("DOUBLE_GRESTORE", BitVec::repeat(is_double_grestore, 1));
        }

        return;
    }

    if edev.chip.kind == ChipKind::Spartan3 {
        for tile in [ll, ul, lr, ur] {
            for bel in ["DCIRESET0", "DCIRESET1"] {
                let diff = ctx.state.get_diff(tile, bel, "PRESENT", "1");
                ctx.tiledb.insert(tile, bel, "ENABLE", xlat_bit(diff));
            }
        }
    }

    // LL
    let tile = ll;
    let bel = "MISC";
    if edev.chip.kind.is_virtex2() {
        ctx.collect_enum_bool(tile, bel, "DISABLEBANDGAP", "NO", "YES");
        ctx.collect_enum_bool_wide(tile, bel, "RAISEVGG", "NO", "YES");
        let item = xlat_bitvec(vec![
            ctx.state.get_diff(tile, bel, "ZCLK_N2", "1"),
            ctx.state.get_diff(tile, bel, "ZCLK_N4", "1"),
            ctx.state.get_diff(tile, bel, "ZCLK_N8", "1"),
            ctx.state.get_diff(tile, bel, "ZCLK_N16", "1"),
            ctx.state.get_diff(tile, bel, "ZCLK_N32", "1"),
        ]);
        ctx.tiledb.insert(tile, bel, "ZCLK_DIV2", item);
        let item = xlat_bitvec(vec![
            ctx.state.get_diff(tile, bel, "IBCLK_N2", "1"),
            ctx.state.get_diff(tile, bel, "IBCLK_N4", "1"),
            ctx.state.get_diff(tile, bel, "IBCLK_N8", "1"),
            ctx.state.get_diff(tile, bel, "IBCLK_N16", "1"),
            ctx.state.get_diff(tile, bel, "IBCLK_N32", "1"),
        ]);
        ctx.tiledb.insert(tile, bel, "BCLK_DIV2", item);
        for attr in [
            "ZCLK_N2",
            "ZCLK_N4",
            "ZCLK_N8",
            "ZCLK_N16",
            "ZCLK_N32",
            "IBCLK_N2",
            "IBCLK_N4",
            "IBCLK_N8",
            "IBCLK_N16",
            "IBCLK_N32",
        ] {
            ctx.state.get_diff(tile, bel, attr, "0").assert_empty();
        }
        if edev.chip.kind.is_virtex2p() {
            ctx.collect_enum_bool(tile, bel, "DISABLEVGGGENERATION", "NO", "YES");
        }
    } else {
        let sendmax = ctx.collect_enum_bool_default(tile, bel, "VGG_SENDMAX", "NO", "YES");
        ctx.insert_device_data("MISC:VGG_SENDMAX_DEFAULT", [sendmax]);
        assert!(!ctx.collect_enum_bool_default(tile, bel, "VGG_ENABLE_OFFCHIP", "NO", "YES"));
        let (item0, vgg0) = ctx.extract_enum_bool_default(tile, bel, "SEND_VGG0", "0", "1");
        let (item1, vgg1) = ctx.extract_enum_bool_default(tile, bel, "SEND_VGG1", "0", "1");
        let (item2, vgg2) = ctx.extract_enum_bool_default(tile, bel, "SEND_VGG2", "0", "1");
        let (item3, vgg3) = ctx.extract_enum_bool_default(tile, bel, "SEND_VGG3", "0", "1");
        ctx.insert_device_data("MISC:SEND_VGG_DEFAULT", [vgg0, vgg1, vgg2, vgg3]);
        let item = concat_bitvec([item0, item1, item2, item3]);
        ctx.tiledb.insert(tile, bel, "SEND_VGG", item);
        if edev.chip.kind.is_spartan3a() {
            let tile = "REG.COR1.S3A";
            let sendmax = ctx.collect_enum_bool_default(tile, bel, "VGG_SENDMAX", "NO", "YES");
            ctx.insert_device_data("MISC:VGG_SENDMAX_DEFAULT", [sendmax]);
            assert!(!ctx.collect_enum_bool_default(tile, bel, "VGG_ENABLE_OFFCHIP", "NO", "YES"));
            let (item0, vgg0) = ctx.extract_enum_bool_default(tile, bel, "SEND_VGG0", "0", "1");
            let (item1, vgg1) = ctx.extract_enum_bool_default(tile, bel, "SEND_VGG1", "0", "1");
            let (item2, vgg2) = ctx.extract_enum_bool_default(tile, bel, "SEND_VGG2", "0", "1");
            let (item3, vgg3) = ctx.extract_enum_bool_default(tile, bel, "SEND_VGG3", "0", "1");
            ctx.insert_device_data("MISC:SEND_VGG_DEFAULT", [vgg0, vgg1, vgg2, vgg3]);
            let item = concat_bitvec([item0, item1, item2, item3]);
            ctx.tiledb.insert(tile, bel, "SEND_VGG", item);
        }
    }
    if edev.chip.kind == ChipKind::Spartan3 {
        let item = xlat_bitvec(vec![
            ctx.state.get_diff(tile, bel, "IDCI_OSC_SEL0", "1"),
            ctx.state.get_diff(tile, bel, "IDCI_OSC_SEL1", "1"),
            ctx.state.get_diff(tile, bel, "IDCI_OSC_SEL2", "1"),
        ]);
        ctx.tiledb.insert(tile, bel, "DCI_OSC_SEL", item);
        for attr in ["IDCI_OSC_SEL0", "IDCI_OSC_SEL1", "IDCI_OSC_SEL2"] {
            ctx.state.get_diff(tile, bel, attr, "0").assert_empty();
        }
        ctx.collect_enum_bool(tile, bel, "GATE_GHIGH", "NO", "YES");
    }
    if edev.chip.kind.is_spartan3ea() {
        ctx.collect_enum(
            tile,
            bel,
            "TEMPSENSOR",
            &["NONE", "PGATE", "CGATE", "BG", "THERM"],
        );
    }
    if edev.chip.kind.is_spartan3a() {
        ctx.collect_enum(tile, bel, "CCLK2PIN", &["PULLDOWN", "PULLUP", "PULLNONE"]);
        ctx.collect_enum(tile, bel, "MOSI2PIN", &["PULLDOWN", "PULLUP", "PULLNONE"]);
    } else if edev.chip.kind != ChipKind::Spartan3E && edev.chip.kind != ChipKind::FpgaCore {
        ctx.collect_enum(tile, bel, "M0PIN", &["PULLDOWN", "PULLUP", "PULLNONE"]);
        ctx.collect_enum(tile, bel, "M1PIN", &["PULLDOWN", "PULLUP", "PULLNONE"]);
        ctx.collect_enum(tile, bel, "M2PIN", &["PULLDOWN", "PULLUP", "PULLNONE"]);
    }
    if edev.chip.kind.is_virtex2() {
        let diff = ctx.state.get_diff(tile, bel, "FREEZE_DCI", "1");
        let diff = diff.filter_tiles(&[4]);
        let mut freeze_dci_nops = 0;
        for (bit, val) in diff.bits {
            assert!(val);
            freeze_dci_nops |= 1 << bit.bit;
        }
        ctx.insert_device_data("FREEZE_DCI_NOPS", freeze_dci_nops);
    }

    // UL
    let tile = ul;
    let bel = "MISC";
    if edev.chip.kind != ChipKind::FpgaCore {
        ctx.collect_enum(tile, bel, "PROGPIN", &["PULLUP", "PULLNONE"]);
        ctx.collect_enum(tile, bel, "TDIPIN", &["PULLDOWN", "PULLUP", "PULLNONE"]);
    }
    if edev.chip.kind.is_spartan3a() {
        ctx.collect_enum(tile, bel, "TMSPIN", &["PULLDOWN", "PULLUP", "PULLNONE"]);
    }
    if !edev.chip.kind.is_spartan3ea() && edev.chip.kind != ChipKind::FpgaCore {
        ctx.collect_enum(tile, bel, "HSWAPENPIN", &["PULLDOWN", "PULLUP", "PULLNONE"]);
    }
    ctx.collect_enum_bool(tile, bel, "TEST_LL", "NO", "YES");

    ctx.state
        .get_diff(tile, "PMV", "PRESENT", "1")
        .assert_empty();
    if edev.chip.kind.is_spartan3a() {
        ctx.state
            .get_diff(tile, "DNA_PORT", "PRESENT", "1")
            .assert_empty();
    }

    // LR
    let tile = lr;
    let bel = "MISC";
    if edev.chip.kind != ChipKind::FpgaCore {
        ctx.collect_enum(tile, bel, "DONEPIN", &["PULLUP", "PULLNONE"]);
    }
    if !edev.chip.kind.is_spartan3a() && edev.chip.kind != ChipKind::FpgaCore {
        ctx.collect_enum(tile, bel, "CCLKPIN", &["PULLUP", "PULLNONE"]);
    }
    if edev.chip.kind.is_virtex2() {
        ctx.collect_enum(tile, bel, "POWERDOWNPIN", &["PULLUP", "PULLNONE"]);
    }
    if edev.chip.kind == ChipKind::FpgaCore {
        let mut items = vec![];
        for attr in ["ABUFF0", "ABUFF1", "ABUFF2", "ABUFF3"] {
            items.push(ctx.extract_enum_bool(tile, bel, attr, "0", "1"));
        }
        ctx.tiledb.insert(tile, bel, "ABUFF", concat_bitvec(items));
    }
    let bel = "STARTUP";
    ctx.state.get_diff(tile, bel, "PRESENT", "1").assert_empty();
    let item = ctx.extract_enum_bool(int_tiles[0], bel, "CLKINV", "CLK", "CLK_B");
    ctx.insert_int_inv(int_tiles, tile, bel, "CLK", item);
    let item = if edev.chip.kind.is_virtex2() {
        // caution: invert
        ctx.extract_enum_bool(int_tiles[0], bel, "GTSINV", "GTS_B", "GTS")
    } else {
        ctx.extract_enum_bool(int_tiles[0], bel, "GTSINV", "GTS", "GTS_B")
    };
    ctx.insert_int_inv(int_tiles, tile, bel, "GTS", item);
    let item = if edev.chip.kind.is_virtex2() {
        // caution: invert
        ctx.extract_enum_bool(int_tiles[0], bel, "GSRINV", "GSR_B", "GSR")
    } else {
        ctx.extract_enum_bool(int_tiles[0], bel, "GSRINV", "GSR", "GSR_B")
    };
    ctx.insert_int_inv(int_tiles, tile, bel, "GSR", item);
    let diff0_gts = ctx.state.get_diff(tile, bel, "GTSINV", "GTS");
    let diff1_gts = ctx.state.get_diff(tile, bel, "GTSINV", "GTS_B");
    assert_eq!(diff0_gts, diff1_gts);
    let diff0_gsr = ctx.state.get_diff(tile, bel, "GSRINV", "GSR");
    let diff1_gsr = ctx.state.get_diff(tile, bel, "GSRINV", "GSR_B");
    assert_eq!(diff0_gsr, diff1_gsr);
    assert_eq!(diff0_gts, diff0_gsr);
    ctx.tiledb
        .insert(tile, bel, "GTS_GSR_ENABLE", xlat_bit(diff0_gsr));
    ctx.collect_enum_bool(tile, bel, "GTS_SYNC", "NO", "YES");
    ctx.collect_enum_bool(tile, bel, "GSR_SYNC", "NO", "YES");
    if edev.chip.kind.is_virtex2() {
        ctx.collect_enum_bool(tile, bel, "GWE_SYNC", "NO", "YES");
    }
    let bel = "CAPTURE";
    ctx.state.get_diff(tile, bel, "PRESENT", "1").assert_empty();
    let item = ctx.extract_enum_bool(int_tiles[0], bel, "CLKINV", "CLK", "CLK_B");
    ctx.insert_int_inv(int_tiles, tile, bel, "CLK", item);
    let item = if edev.chip.kind.is_virtex2() {
        // caution: inverted
        ctx.extract_enum_bool(int_tiles[0], bel, "CAPINV", "CAP_B", "CAP")
    } else {
        ctx.extract_enum_bool(int_tiles[0], bel, "CAPINV", "CAP", "CAP_B")
    };
    ctx.insert_int_inv(int_tiles, tile, bel, "CAP", item);
    let bel = "ICAP";
    if edev.chip.kind != ChipKind::Spartan3E {
        let item = ctx.extract_enum_bool(int_tiles[0], bel, "CLKINV", "CLK", "CLK_B");
        ctx.insert_int_inv(int_tiles, tile, bel, "CLK", item);
        let item = if edev.chip.kind.is_virtex2() {
            ctx.extract_enum_bool(int_tiles[0], bel, "CEINV", "CE", "CE_B")
        } else {
            // caution: inverted
            ctx.extract_enum_bool(int_tiles[0], bel, "CEINV", "CE_B", "CE")
        };
        ctx.insert_int_inv(int_tiles, tile, bel, "CE", item);
        let item = if edev.chip.kind.is_virtex2() {
            ctx.extract_enum_bool(int_tiles[0], bel, "WRITEINV", "WRITE", "WRITE_B")
        } else {
            // caution: inverted
            ctx.extract_enum_bool(int_tiles[0], bel, "WRITEINV", "WRITE_B", "WRITE")
        };
        ctx.insert_int_inv(int_tiles, tile, bel, "WRITE", item);
        if !edev.chip.kind.is_spartan3a() {
            ctx.collect_bit(tile, bel, "ENABLE", "1");
        }
    }
    if edev.chip.kind.is_spartan3a() {
        let bel = "SPI_ACCESS";
        ctx.collect_bit(tile, bel, "ENABLE", "1");
        let mut diff = ctx.state.get_diff(int_tiles[0], bel, "ENABLE", "1");
        diff.discard_bits(&ctx.item_int_inv(int_tiles, tile, bel, "MOSI"));
        diff.assert_empty();
    }

    // UR
    let tile = ur;
    let bel = "MISC";
    if edev.chip.kind != ChipKind::FpgaCore {
        ctx.collect_enum(tile, bel, "TCKPIN", &["PULLDOWN", "PULLUP", "PULLNONE"]);
        ctx.collect_enum(tile, bel, "TDOPIN", &["PULLDOWN", "PULLUP", "PULLNONE"]);
        if !edev.chip.kind.is_spartan3a() {
            ctx.collect_enum(tile, bel, "TMSPIN", &["PULLDOWN", "PULLUP", "PULLNONE"]);
        } else {
            ctx.collect_enum(tile, bel, "MISO2PIN", &["PULLDOWN", "PULLUP", "PULLNONE"]);
            ctx.collect_enum(tile, bel, "CSO2PIN", &["PULLDOWN", "PULLUP", "PULLNONE"]);
        }
    }
    if edev.chip.kind.is_virtex2() {
        ctx.collect_enum_bool(tile, bel, "TEST_LL", "NO", "YES");
    }
    let bel = "BSCAN";
    ctx.state.get_diff(tile, bel, "PRESENT", "1").assert_empty();
    ctx.collect_bitvec(tile, bel, "USERID", "");
    let diff = ctx.state.get_diff(tile, bel, "TDO1", "1");
    assert_eq!(diff, ctx.state.get_diff(tile, bel, "TDO2", "1"));
    let mut bits: Vec<_> = diff.bits.into_iter().collect();
    bits.sort();
    ctx.tiledb.insert(
        tile,
        bel,
        "TDO_ENABLE",
        xlat_bitvec(
            bits.into_iter()
                .map(|(k, v)| Diff {
                    bits: [(k, v)].into_iter().collect(),
                })
                .collect(),
        ),
    );

    if edev.chip.kind.is_virtex2p() {
        let bel = "JTAGPPC";
        let diff = ctx.state.get_diff(tile, bel, "PRESENT", "1");
        ctx.tiledb.insert(tile, bel, "ENABLE", xlat_bit(diff));
    }

    if edev.chip.kind == ChipKind::FpgaCore {
        for tile in ["LL.FC", "UL.FC", "LR.FC", "UR.FC"] {
            let bel = "MISC";
            ctx.collect_bit(tile, bel, "MISR_CLOCK", "GCLK0");
            ctx.collect_enum_bool(tile, bel, "MISR_RESET", "NO", "YES");
        }
        // could be verified, but meh; they just route given GCLK to CLK3 of every corner tile.
        ctx.state.get_diff("INT.CLB", "MISC", "MISR_CLOCK", "GCLK0");
        ctx.state.get_diff("GCLKH", "MISC", "MISR_CLOCK", "GCLK0");
        ctx.state
            .get_diff("GCLKVM.S3", "MISC", "MISR_CLOCK", "GCLK0");
    }

    // I/O bank misc control
    if !skip_io && edev.chip.kind != ChipKind::FpgaCore {
        if !edev.chip.kind.is_spartan3ea() {
            for (tile, bel) in [
                (ul, "DCI0"),
                (ul, "DCI1"),
                (ur, "DCI1"),
                (ur, "DCI0"),
                (lr, "DCI0"),
                (lr, "DCI1"),
                (ll, "DCI1"),
                (ll, "DCI0"),
            ] {
                // LVDS
                let mut vals = vec![];
                for std in get_iostds(edev, false) {
                    if std.diff != DiffKind::True {
                        continue;
                    }
                    let diff = ctx.state.get_diff(tile, bel, "LVDSBIAS", std.name);
                    vals.push((
                        std.name,
                        diff.filter_tiles(if edev.chip.kind.is_virtex2() {
                            &[0, 1][..]
                        } else {
                            &[0][..]
                        }),
                    ));
                }
                vals.push(("OFF", Diff::default()));
                let prefix = match edev.chip.kind {
                    ChipKind::Virtex2 => "IOSTD:V2:LVDSBIAS",
                    ChipKind::Virtex2P | ChipKind::Virtex2PX => "IOSTD:V2P:LVDSBIAS",
                    ChipKind::Spartan3 => "IOSTD:S3:LVDSBIAS",
                    _ => unreachable!(),
                };

                let item = if edev.chip.kind == ChipKind::Spartan3 {
                    xlat_bitvec(
                        (0..13)
                            .rev()
                            .map(|i| {
                                ctx.state
                                    .get_diff(tile, bel, format!("LVDSBIAS_OPT{i}"), "1")
                            })
                            .collect(),
                    )
                } else {
                    TileItem::from_bitvec(
                        match bel {
                            "DCI0" => vec![
                                TileBit::new(0, 3, 48),
                                TileBit::new(0, 2, 48),
                                TileBit::new(0, 3, 47),
                                TileBit::new(0, 2, 47),
                                TileBit::new(0, 3, 46),
                                TileBit::new(0, 2, 46),
                                TileBit::new(0, 3, 45),
                                TileBit::new(0, 2, 45),
                                TileBit::new(0, 3, 44),
                            ],
                            "DCI1" => vec![
                                TileBit::new(1, 12, 8),
                                TileBit::new(1, 12, 6),
                                TileBit::new(1, 12, 7),
                                TileBit::new(1, 12, 10),
                                TileBit::new(1, 12, 11),
                                TileBit::new(1, 12, 9),
                                TileBit::new(1, 13, 9),
                                TileBit::new(1, 13, 11),
                                TileBit::new(1, 13, 7),
                            ],
                            _ => unreachable!(),
                        },
                        false,
                    )
                };
                let base = BitVec::repeat(false, item.bits.len());
                for (name, diff) in vals {
                    let val = extract_bitvec_val(&item, &base, diff);
                    ctx.tiledb.insert_misc_data(format!("{prefix}:{name}"), val)
                }
                ctx.tiledb.insert(tile, bel, "LVDSBIAS", item);

                // DCI
                let diff_fdh = !ctx.state.get_diff(tile, bel, "FORCE_DONE_HIGH", "#OFF");
                if edev.chip.kind.is_virtex2() {
                    let diff = ctx
                        .state
                        .get_diff(tile, bel, "DCI_OUT", "1")
                        .filter_tiles(&[0, 1]);
                    let diff_p = ctx.state.get_diff(tile, bel, "PRESENT", "1");
                    let diff_t = ctx.state.get_diff(tile, bel, "PRESENT", "TEST");
                    assert_eq!(diff_p, diff.combine(&diff_fdh));
                    ctx.tiledb.insert(tile, bel, "ENABLE", xlat_bit(diff));
                    let diff_t = diff_t.combine(&!diff_p);
                    ctx.tiledb
                        .insert(tile, bel, "TEST_ENABLE", xlat_bit(diff_t));
                } else {
                    let diff_ar = ctx
                        .state
                        .get_diff(tile, bel, "DCI_OUT", "ASREQUIRED")
                        .filter_tiles(&[0]);
                    let diff_c = ctx
                        .state
                        .get_diff(tile, bel, "DCI_OUT", "CONTINUOUS")
                        .filter_tiles(&[0]);
                    let diff_q = ctx
                        .state
                        .get_diff(tile, bel, "DCI_OUT", "QUIET")
                        .filter_tiles(&[0]);
                    let diff_p = ctx.state.get_diff(tile, bel, "PRESENT", "1");
                    assert_eq!(diff_c, diff_ar);
                    let diff_q = diff_q.combine(&!&diff_c);
                    let diff_p = diff_p.combine(&!&diff_c).combine(&!&diff_fdh);
                    ctx.tiledb.insert(tile, bel, "ENABLE", xlat_bit(diff_c));
                    ctx.tiledb.insert(tile, bel, "QUIET", xlat_bit(diff_q));
                    ctx.tiledb
                        .insert(tile, bel, "TEST_ENABLE", xlat_bit(diff_p));
                }
                ctx.tiledb
                    .insert(tile, bel, "FORCE_DONE_HIGH", xlat_bit(diff_fdh));

                // DCI TERM stuff
                let (pmask_term_vcc, pmask_term_split, nmask_term_split) =
                    if edev.chip.kind == ChipKind::Spartan3 {
                        let frame = if tile == ll {
                            match bel {
                                "DCI0" => 1,
                                "DCI1" => 0,
                                _ => unreachable!(),
                            }
                        } else {
                            match bel {
                                "DCI0" => 0,
                                "DCI1" => 1,
                                _ => unreachable!(),
                            }
                        };
                        (
                            TileItem::from_bitvec(
                                vec![
                                    TileBit::new(0, frame, 51),
                                    TileBit::new(0, frame, 52),
                                    TileBit::new(0, frame, 53),
                                    TileBit::new(0, frame, 54),
                                ],
                                false,
                            ),
                            TileItem::from_bitvec(
                                vec![
                                    TileBit::new(0, frame, 56),
                                    TileBit::new(0, frame, 57),
                                    TileBit::new(0, frame, 58),
                                    TileBit::new(0, frame, 59),
                                ],
                                false,
                            ),
                            TileItem::from_bitvec(
                                vec![
                                    TileBit::new(0, frame, 46),
                                    TileBit::new(0, frame, 47),
                                    TileBit::new(0, frame, 48),
                                    TileBit::new(0, frame, 49),
                                ],
                                false,
                            ),
                        )
                    } else {
                        (
                            TileItem::from_bitvec(
                                match bel {
                                    "DCI0" => vec![
                                        TileBit::new(0, 3, 36),
                                        TileBit::new(0, 2, 36),
                                        TileBit::new(0, 3, 35),
                                        TileBit::new(0, 2, 35),
                                        TileBit::new(0, 3, 34),
                                    ],
                                    "DCI1" => vec![
                                        TileBit::new(1, 8, 8),
                                        TileBit::new(1, 8, 6),
                                        TileBit::new(1, 8, 7),
                                        TileBit::new(1, 8, 11),
                                        TileBit::new(1, 8, 10),
                                    ],
                                    _ => unreachable!(),
                                },
                                false,
                            ),
                            TileItem::from_bitvec(
                                match bel {
                                    "DCI0" => vec![
                                        TileBit::new(0, 2, 34),
                                        TileBit::new(0, 3, 33),
                                        TileBit::new(0, 2, 33),
                                        TileBit::new(0, 3, 32),
                                        TileBit::new(0, 2, 32),
                                    ],
                                    "DCI1" => vec![
                                        TileBit::new(1, 8, 9),
                                        TileBit::new(1, 9, 9),
                                        TileBit::new(1, 9, 11),
                                        TileBit::new(1, 9, 7),
                                        TileBit::new(1, 9, 10),
                                    ],
                                    _ => unreachable!(),
                                },
                                false,
                            ),
                            TileItem::from_bitvec(
                                match bel {
                                    "DCI0" => vec![
                                        TileBit::new(0, 2, 39),
                                        TileBit::new(0, 3, 38),
                                        TileBit::new(0, 2, 38),
                                        TileBit::new(0, 3, 37),
                                        TileBit::new(0, 2, 37),
                                    ],
                                    "DCI1" => vec![
                                        TileBit::new(1, 11, 11),
                                        TileBit::new(1, 11, 7),
                                        TileBit::new(1, 11, 10),
                                        TileBit::new(1, 11, 8),
                                        TileBit::new(1, 11, 6),
                                    ],
                                    _ => unreachable!(),
                                },
                                false,
                            ),
                        )
                    };
                let item_en = ctx.tiledb.item(tile, bel, "ENABLE").clone();
                let prefix = match edev.chip.kind {
                    ChipKind::Virtex2 => "IOSTD:V2",
                    ChipKind::Virtex2P | ChipKind::Virtex2PX => "IOSTD:V2P",
                    ChipKind::Spartan3 => "IOSTD:S3",
                    _ => unreachable!(),
                };
                for std in get_iostds(edev, false) {
                    if std.name.starts_with("DIFF_") {
                        continue;
                    }
                    match std.dci {
                        DciKind::None | DciKind::Output | DciKind::OutputHalf => (),
                        DciKind::InputVcc | DciKind::BiVcc => {
                            let mut diff = ctx
                                .state
                                .get_diff(tile, bel, "DCI_TERM", std.name)
                                .filter_tiles(if edev.chip.kind.is_virtex2() {
                                    &[0, 1][..]
                                } else {
                                    &[0][..]
                                });
                            diff.apply_bit_diff(&item_en, true, false);
                            let val = extract_bitvec_val_part(
                                &pmask_term_vcc,
                                &BitVec::repeat(false, pmask_term_vcc.bits.len()),
                                &mut diff,
                            );
                            ctx.tiledb.insert_misc_data(
                                format!("{prefix}:PMASK_TERM_VCC:{stdname}", stdname = std.name),
                                val,
                            );
                            diff.assert_empty();
                        }
                        DciKind::InputSplit | DciKind::BiSplit => {
                            if std.diff == DiffKind::True {
                                ctx.tiledb.insert_misc_data(
                                    format!(
                                        "{prefix}:PMASK_TERM_SPLIT:{stdname}",
                                        stdname = std.name
                                    ),
                                    BitVec::repeat(false, pmask_term_split.bits.len()),
                                );
                                ctx.tiledb.insert_misc_data(
                                    format!(
                                        "{prefix}:NMASK_TERM_SPLIT:{stdname}",
                                        stdname = std.name
                                    ),
                                    BitVec::repeat(false, nmask_term_split.bits.len()),
                                );
                            } else {
                                let mut diff = ctx
                                    .state
                                    .get_diff(tile, bel, "DCI_TERM", std.name)
                                    .filter_tiles(if edev.chip.kind.is_virtex2() {
                                        &[0, 1][..]
                                    } else {
                                        &[0][..]
                                    });
                                diff.apply_bit_diff(&item_en, true, false);
                                let val = extract_bitvec_val_part(
                                    &pmask_term_split,
                                    &BitVec::repeat(false, pmask_term_split.bits.len()),
                                    &mut diff,
                                );
                                ctx.tiledb.insert_misc_data(
                                    format!(
                                        "{prefix}:PMASK_TERM_SPLIT:{stdname}",
                                        stdname = std.name
                                    ),
                                    val,
                                );
                                let val = extract_bitvec_val_part(
                                    &nmask_term_split,
                                    &BitVec::repeat(false, nmask_term_split.bits.len()),
                                    &mut diff,
                                );
                                ctx.tiledb.insert_misc_data(
                                    format!(
                                        "{prefix}:NMASK_TERM_SPLIT:{stdname}",
                                        stdname = std.name
                                    ),
                                    val,
                                );
                                diff.assert_empty();
                            }
                        }
                        _ => unreachable!(),
                    }
                }
                ctx.tiledb.insert_misc_data(
                    format!("{prefix}:PMASK_TERM_VCC:OFF"),
                    BitVec::repeat(false, pmask_term_vcc.bits.len()),
                );
                ctx.tiledb.insert_misc_data(
                    format!("{prefix}:PMASK_TERM_SPLIT:OFF"),
                    BitVec::repeat(false, pmask_term_split.bits.len()),
                );
                ctx.tiledb.insert_misc_data(
                    format!("{prefix}:NMASK_TERM_SPLIT:OFF"),
                    BitVec::repeat(false, nmask_term_split.bits.len()),
                );

                ctx.tiledb
                    .insert(tile, bel, "PMASK_TERM_VCC", pmask_term_vcc);
                ctx.tiledb
                    .insert(tile, bel, "PMASK_TERM_SPLIT", pmask_term_split);
                ctx.tiledb
                    .insert(tile, bel, "NMASK_TERM_SPLIT", nmask_term_split);
            }

            if edev.chip.kind == ChipKind::Spartan3 {
                for tile in [ll, ul, lr, ur] {
                    let item = xlat_enum(vec![
                        ("DCI0", ctx.state.get_diff(tile, "DCI0", "SELECT", "1")),
                        ("DCI1", ctx.state.get_diff(tile, "DCI1", "SELECT", "1")),
                    ]);
                    ctx.tiledb.insert(tile, "MISC", "DCI_TEST_MUX", item);
                }
            }
            if edev.chip.kind.is_virtex2p()
                && !ctx.device.name.ends_with("2vp4")
                && !ctx.device.name.ends_with("2vp7")
            {
                ctx.state
                    .get_diff(ll, "MISC", "DCIUPDATEMODE", "ASREQUIRED")
                    .assert_empty();
                ctx.state
                    .get_diff(ll, "MISC", "DCIUPDATEMODE", "CONTINUOUS")
                    .assert_empty();
                let diff = ctx.state.get_diff(ll, "MISC", "DCIUPDATEMODE", "QUIET");
                let diff0 = diff.filter_tiles(&[8, 0]);
                let diff1 = diff.filter_tiles(&[8, 1]);
                let diff2 = diff.filter_tiles(&[2]);
                let diff3 = diff.filter_tiles(&[3]);
                let diff4 = diff.filter_tiles(&[8, 4]);
                let diff5 = diff.filter_tiles(&[8, 5]);
                let diff6 = diff.filter_tiles(&[6]);
                let diff7 = diff.filter_tiles(&[7]);
                ctx.tiledb.insert(ul, "DCI1", "QUIET", xlat_bit(diff0));
                ctx.tiledb.insert(ur, "DCI1", "QUIET", xlat_bit(diff1));
                ctx.tiledb.insert(ur, "DCI0", "QUIET", xlat_bit(diff2));
                ctx.tiledb.insert(lr, "DCI0", "QUIET", xlat_bit(diff3));
                ctx.tiledb.insert(lr, "DCI1", "QUIET", xlat_bit(diff4));
                ctx.tiledb.insert(ll, "DCI1", "QUIET", xlat_bit(diff5));
                ctx.tiledb.insert(ll, "DCI0", "QUIET", xlat_bit(diff6));
                ctx.tiledb.insert(ul, "DCI0", "QUIET", xlat_bit(diff7));
            }

            let tile = ll;
            let bel = "DCI0";
            let mut diff = ctx
                .state
                .get_diff(tile, bel, "DCI_OUT_ALONE", "1")
                .filter_tiles(if edev.chip.kind.is_virtex2() {
                    &[0, 1][..]
                } else {
                    &[0][..]
                });
            diff.apply_bit_diff(ctx.tiledb.item(tile, bel, "ENABLE"), true, false);
            if edev.chip.dci_io_alt.contains_key(&5) {
                let bel = "DCI1";
                let mut alt_diff = ctx
                    .state
                    .get_diff(tile, bel, "DCI_OUT_ALONE", "1")
                    .filter_tiles(if edev.chip.kind.is_virtex2() {
                        &[0, 1][..]
                    } else {
                        &[0][..]
                    });
                alt_diff.apply_bit_diff(ctx.tiledb.item(tile, bel, "ENABLE"), true, false);
                alt_diff = alt_diff.combine(&!&diff);
                ctx.tiledb
                    .insert(tile, "MISC", "DCI_ALTVR", xlat_bit(alt_diff));
            }
            if edev.chip.kind.is_virtex2() {
                diff.apply_bitvec_diff(
                    ctx.tiledb.item(tile, "MISC", "ZCLK_DIV2"),
                    &bits![0, 0, 0, 1, 0],
                    &BitVec::repeat(false, 5),
                );
            }
            ctx.tiledb
                .insert(tile, "MISC", "DCI_CLK_ENABLE", xlat_bit(diff));
        } else {
            let banks = if edev.chip.kind == ChipKind::Spartan3E {
                vec![
                    (
                        ul,
                        TileItem::from_bitvec(
                            vec![
                                TileBit::new(0, 0, 44),
                                TileBit::new(0, 0, 39),
                                TileBit::new(0, 0, 38),
                                TileBit::new(0, 0, 37),
                                TileBit::new(0, 0, 36),
                                TileBit::new(0, 0, 27),
                                TileBit::new(0, 0, 26),
                                TileBit::new(0, 0, 25),
                                TileBit::new(0, 0, 24),
                                TileBit::new(0, 0, 23),
                                TileBit::new(0, 0, 22),
                            ],
                            false,
                        ),
                        TileItem::from_bitvec(
                            vec![
                                TileBit::new(0, 0, 45),
                                TileBit::new(0, 0, 43),
                                TileBit::new(0, 0, 42),
                                TileBit::new(0, 0, 41),
                                TileBit::new(0, 0, 40),
                                TileBit::new(0, 0, 35),
                                TileBit::new(0, 0, 34),
                                TileBit::new(0, 0, 33),
                                TileBit::new(0, 0, 32),
                                TileBit::new(0, 0, 29),
                                TileBit::new(0, 0, 28),
                            ],
                            false,
                        ),
                    ),
                    (
                        ur,
                        TileItem::from_bitvec(
                            vec![
                                TileBit::new(0, 1, 10),
                                TileBit::new(0, 1, 48),
                                TileBit::new(0, 1, 47),
                                TileBit::new(0, 1, 46),
                                TileBit::new(0, 1, 45),
                                TileBit::new(0, 1, 38),
                                TileBit::new(0, 1, 37),
                                TileBit::new(0, 1, 36),
                                TileBit::new(0, 1, 35),
                                TileBit::new(0, 1, 34),
                                TileBit::new(0, 1, 33),
                            ],
                            false,
                        ),
                        TileItem::from_bitvec(
                            vec![
                                TileBit::new(0, 1, 11),
                                TileBit::new(0, 1, 9),
                                TileBit::new(0, 1, 51),
                                TileBit::new(0, 1, 50),
                                TileBit::new(0, 1, 49),
                                TileBit::new(0, 1, 44),
                                TileBit::new(0, 1, 43),
                                TileBit::new(0, 1, 42),
                                TileBit::new(0, 1, 41),
                                TileBit::new(0, 1, 40),
                                TileBit::new(0, 1, 39),
                            ],
                            false,
                        ),
                    ),
                    (
                        lr,
                        TileItem::from_bitvec(
                            vec![
                                TileBit::new(0, 1, 12),
                                TileBit::new(0, 1, 7),
                                TileBit::new(0, 1, 36),
                                TileBit::new(0, 1, 35),
                                TileBit::new(0, 1, 34),
                                TileBit::new(0, 1, 27),
                                TileBit::new(0, 1, 26),
                                TileBit::new(0, 1, 25),
                                TileBit::new(0, 1, 24),
                                TileBit::new(0, 1, 23),
                                TileBit::new(0, 1, 22),
                            ],
                            false,
                        ),
                        TileItem::from_bitvec(
                            vec![
                                TileBit::new(0, 1, 13),
                                TileBit::new(0, 1, 11),
                                TileBit::new(0, 1, 10),
                                TileBit::new(0, 1, 9),
                                TileBit::new(0, 1, 8),
                                TileBit::new(0, 1, 33),
                                TileBit::new(0, 1, 32),
                                TileBit::new(0, 1, 31),
                                TileBit::new(0, 1, 30),
                                TileBit::new(0, 1, 29),
                                TileBit::new(0, 1, 28),
                            ],
                            false,
                        ),
                    ),
                    (
                        ll,
                        TileItem::from_bitvec(
                            vec![
                                TileBit::new(0, 1, 31),
                                TileBit::new(0, 1, 26),
                                TileBit::new(0, 1, 25),
                                TileBit::new(0, 1, 24),
                                TileBit::new(0, 1, 23),
                                TileBit::new(0, 1, 38),
                                TileBit::new(0, 1, 37),
                                TileBit::new(0, 1, 36),
                                TileBit::new(0, 1, 35),
                                TileBit::new(0, 1, 34),
                                TileBit::new(0, 1, 33),
                            ],
                            false,
                        ),
                        TileItem::from_bitvec(
                            vec![
                                TileBit::new(0, 1, 32),
                                TileBit::new(0, 1, 30),
                                TileBit::new(0, 1, 29),
                                TileBit::new(0, 1, 28),
                                TileBit::new(0, 1, 27),
                                TileBit::new(0, 1, 22),
                                TileBit::new(0, 1, 43),
                                TileBit::new(0, 1, 42),
                                TileBit::new(0, 1, 41),
                                TileBit::new(0, 1, 40),
                                TileBit::new(0, 1, 39),
                            ],
                            false,
                        ),
                    ),
                ]
            } else {
                vec![
                    (
                        ul,
                        TileItem::from_bitvec(
                            vec![
                                TileBit::new(0, 1, 62),
                                TileBit::new(0, 1, 60),
                                TileBit::new(0, 1, 55),
                                TileBit::new(0, 1, 54),
                                TileBit::new(0, 1, 53),
                                TileBit::new(0, 1, 52),
                                TileBit::new(0, 1, 45),
                                TileBit::new(0, 1, 44),
                                TileBit::new(0, 1, 43),
                                TileBit::new(0, 1, 42),
                                TileBit::new(0, 1, 41),
                                TileBit::new(0, 1, 40),
                            ],
                            false,
                        ),
                        TileItem::from_bitvec(
                            vec![
                                TileBit::new(0, 1, 63),
                                TileBit::new(0, 1, 61),
                                TileBit::new(0, 1, 59),
                                TileBit::new(0, 1, 58),
                                TileBit::new(0, 1, 57),
                                TileBit::new(0, 1, 56),
                                TileBit::new(0, 1, 51),
                                TileBit::new(0, 1, 50),
                                TileBit::new(0, 1, 49),
                                TileBit::new(0, 1, 48),
                                TileBit::new(0, 1, 47),
                                TileBit::new(0, 1, 46),
                            ],
                            false,
                        ),
                    ),
                    (
                        ll,
                        TileItem::from_bitvec(
                            vec![
                                TileBit::new(0, 1, 32),
                                TileBit::new(0, 0, 27),
                                TileBit::new(0, 0, 31),
                                TileBit::new(0, 1, 30),
                                TileBit::new(0, 1, 36),
                                TileBit::new(0, 1, 28),
                                TileBit::new(0, 0, 10),
                                TileBit::new(0, 1, 11),
                                TileBit::new(0, 1, 34),
                                TileBit::new(0, 1, 33),
                                TileBit::new(0, 1, 10),
                                TileBit::new(0, 0, 9),
                            ],
                            false,
                        ),
                        TileItem::from_bitvec(
                            vec![
                                TileBit::new(0, 1, 27),
                                TileBit::new(0, 0, 28),
                                TileBit::new(0, 0, 26),
                                TileBit::new(0, 1, 26),
                                TileBit::new(0, 1, 62),
                                TileBit::new(0, 1, 63),
                                TileBit::new(0, 0, 30),
                                TileBit::new(0, 1, 9),
                                TileBit::new(0, 1, 35),
                                TileBit::new(0, 0, 29),
                                TileBit::new(0, 0, 62),
                                TileBit::new(0, 0, 6),
                            ],
                            false,
                        ),
                    ),
                ]
            };
            for (tile, lvdsbias_0, lvdsbias_1) in banks {
                let bel = "BANK";
                let prefix = if edev.chip.kind == ChipKind::Spartan3E {
                    "IOSTD:S3E:LVDSBIAS"
                } else {
                    "IOSTD:S3A.TB:LVDSBIAS"
                };
                let kind = edev.egrid.db.get_tile_class(tile);
                let tcrd = edev.egrid.tile_index[kind][0];
                let btile = edev.btile_lrterm(tcrd.cell);
                let base: BitVec = lvdsbias_0
                    .bits
                    .iter()
                    .map(|bit| {
                        ctx.empty_bs
                            .get_bit(btile.xlat_pos_fwd((bit.frame, bit.bit)))
                    })
                    .collect();

                for std in get_iostds(edev, false) {
                    if std.diff != DiffKind::True {
                        continue;
                    }
                    if std.name != "LVDS_25" || edev.chip.kind.is_spartan3a() {
                        let diff_0 = ctx
                            .state
                            .get_diff(tile, bel, "LVDSBIAS_0", std.name)
                            .filter_tiles(&[0]);
                        let val_0 = extract_bitvec_val(&lvdsbias_0, &base, diff_0);
                        ctx.tiledb
                            .insert_misc_data(format!("{prefix}:{sn}", sn = std.name), val_0)
                    }
                    let diff_1 = ctx
                        .state
                        .get_diff(tile, bel, "LVDSBIAS_1", std.name)
                        .filter_tiles(&[0]);
                    let val_1 = extract_bitvec_val(&lvdsbias_1, &base, diff_1);
                    ctx.tiledb
                        .insert_misc_data(format!("{prefix}:{sn}", sn = std.name), val_1)
                }
                ctx.tiledb.insert_misc_data(format!("{prefix}:OFF"), base);
                ctx.tiledb.insert(tile, bel, "LVDSBIAS_0", lvdsbias_0);
                ctx.tiledb.insert(tile, bel, "LVDSBIAS_1", lvdsbias_1);
            }
        }

        if edev.chip.kind.is_spartan3ea() {
            for (tile, btile) in [
                (ll, edev.btile_lrterm(edev.chip.corner(DirHV::SW).cell)),
                (ul, edev.btile_lrterm(edev.chip.corner(DirHV::NW).cell)),
                (lr, edev.btile_lrterm(edev.chip.corner(DirHV::SE).cell)),
                (ur, edev.btile_lrterm(edev.chip.corner(DirHV::NE).cell)),
            ] {
                let bel = "MISC";
                let mut diff = Diff::default();
                let BitTile::Main(_, _, width, _, height, _) = btile else {
                    unreachable!()
                };
                for tframe in 0..width {
                    for tbit in 0..height {
                        let bit = btile.xlat_pos_fwd((tframe, tbit));
                        if ctx.empty_bs.get_bit(bit) {
                            diff.bits.insert(
                                TileBit {
                                    tile: 0,
                                    frame: tframe,
                                    bit: tbit,
                                },
                                true,
                            );
                        }
                    }
                }
                if tile == ll {
                    for attr in ["SEND_VGG", "VGG_SENDMAX"] {
                        diff.discard_bits(ctx.tiledb.item(tile, bel, attr));
                    }
                }
                if edev.chip.kind == ChipKind::Spartan3E {
                    for attr in ["LVDSBIAS_0", "LVDSBIAS_1"] {
                        diff.discard_bits(ctx.tiledb.item(tile, "BANK", attr));
                    }
                }
                if !diff.bits.is_empty() {
                    ctx.tiledb
                        .insert(tile, bel, "UNK_ALWAYS_SET", xlat_bit_wide(diff));
                }
            }
        }
    }

    // config regs
    if !edev.chip.kind.is_spartan3a() {
        let tile = if edev.chip.kind.is_virtex2() {
            "REG.COR"
        } else if edev.chip.kind == ChipKind::Spartan3 {
            "REG.COR.S3"
        } else if edev.chip.kind == ChipKind::FpgaCore {
            "REG.COR.FC"
        } else {
            "REG.COR.S3E"
        };
        let bel = "STARTUP";
        ctx.collect_enum(
            tile,
            bel,
            "GWE_CYCLE",
            &["1", "2", "3", "4", "5", "6", "DONE", "KEEP"],
        );
        ctx.collect_enum(
            tile,
            bel,
            "GTS_CYCLE",
            &["1", "2", "3", "4", "5", "6", "DONE", "KEEP"],
        );
        ctx.collect_enum(
            tile,
            bel,
            "DONE_CYCLE",
            &["1", "2", "3", "4", "5", "6", "KEEP"],
        );
        if edev.chip.kind != ChipKind::FpgaCore {
            ctx.collect_enum(
                tile,
                bel,
                "LCK_CYCLE",
                &["0", "1", "2", "3", "4", "5", "6", "NOWAIT"],
            );
        }
        if edev.chip.kind != ChipKind::Spartan3E && edev.chip.kind != ChipKind::FpgaCore {
            ctx.collect_enum(
                tile,
                bel,
                "MATCH_CYCLE",
                &["0", "1", "2", "3", "4", "5", "6", "NOWAIT"],
            );
        }
        ctx.collect_enum(tile, bel, "STARTUPCLK", &["CCLK", "USERCLK", "JTAGCLK"]);
        if edev.chip.kind == ChipKind::Spartan3E {
            ctx.collect_bit(tile, bel, "MULTIBOOT_ENABLE", "1");
        }
        let vals = if edev.chip.kind.is_virtex2() {
            &[
                "4", "5", "7", "8", "9", "10", "13", "15", "20", "26", "30", "34", "41", "51",
                "55", "60", "130",
            ][..]
        } else if !edev.chip.kind.is_spartan3ea() {
            &["3", "6", "12", "25", "50", "100"][..]
        } else {
            &["1", "3", "6", "12", "25", "50"][..]
        };
        ctx.collect_enum_ocd(tile, bel, "CONFIG_RATE", vals, OcdMode::BitOrder);
        if !edev.chip.kind.is_virtex2() {
            ctx.collect_enum(tile, bel, "BUSCLK_FREQ", &["25", "50", "100", "200"]);
        }
        ctx.collect_enum_bool(tile, bel, "DRIVE_DONE", "NO", "YES");
        ctx.collect_enum_bool(tile, bel, "DONE_PIPE", "NO", "YES");
        if edev.chip.kind != ChipKind::FpgaCore {
            ctx.collect_enum_bool(tile, bel, "DCM_SHUTDOWN", "DISABLE", "ENABLE");
        }
        if edev.chip.kind.is_virtex2() {
            ctx.collect_enum_bool(tile, bel, "POWERDOWN_STATUS", "DISABLE", "ENABLE");
            ctx.state
                .get_diff(tile, bel, "DCI_SHUTDOWN", "ENABLE")
                .assert_empty();
            ctx.state
                .get_diff(tile, bel, "DCI_SHUTDOWN", "DISABLE")
                .assert_empty();
        }
        ctx.collect_enum_bool(tile, bel, "CRC", "DISABLE", "ENABLE");
        if matches!(edev.chip.kind, ChipKind::Spartan3 | ChipKind::FpgaCore) {
            ctx.collect_enum(tile, bel, "VRDSEL", &["100", "95", "90", "80"]);
        } else if edev.chip.kind == ChipKind::Spartan3E {
            // ??? 70 == 75?
            let d70 = ctx.state.get_diff(tile, bel, "VRDSEL", "70");
            let d75 = ctx.state.get_diff(tile, bel, "VRDSEL", "75");
            let d80 = ctx.state.get_diff(tile, bel, "VRDSEL", "80");
            let d90 = ctx.state.get_diff(tile, bel, "VRDSEL", "90");
            assert_eq!(d70, d75);
            ctx.tiledb.insert(
                tile,
                bel,
                "VRDSEL",
                xlat_enum_ocd(
                    vec![("70_75", d70), ("80", d80), ("90", d90)],
                    OcdMode::BitOrder,
                ),
            );
        }

        let bel = "CAPTURE";
        let item = ctx.extract_bit(tile, bel, "ONESHOT_ATTR", "ONE_SHOT");
        ctx.tiledb.insert(tile, bel, "ONESHOT", item);

        let tile = if edev.chip.kind.is_virtex2() {
            "REG.CTL"
        } else {
            "REG.CTL.S3"
        };
        let bel = "MISC";
        ctx.collect_enum_bool(tile, bel, "GTS_USR_B", "NO", "YES");
        ctx.collect_enum_bool(tile, bel, "VGG_TEST", "NO", "YES");
        ctx.collect_enum_bool(tile, bel, "BCLK_TEST", "NO", "YES");
        ctx.collect_enum(tile, bel, "SECURITY", &["NONE", "LEVEL1", "LEVEL2"]);
        // these are too much trouble to deal with the normal way.
        ctx.tiledb.insert(
            tile,
            bel,
            "PERSIST",
            TileItem::from_bit(TileBit::new(0, 0, 3), false),
        );
    } else {
        let tile = "REG.COR1.S3A";
        let bel = "STARTUP";
        ctx.collect_enum(tile, bel, "STARTUPCLK", &["CCLK", "USERCLK", "JTAGCLK"]);
        ctx.collect_enum_bool(tile, bel, "DRIVE_DONE", "NO", "YES");
        ctx.collect_enum_bool(tile, bel, "DONE_PIPE", "NO", "YES");
        ctx.collect_enum_bool(tile, bel, "DRIVE_AWAKE", "NO", "YES");
        ctx.collect_enum_bool(tile, bel, "CRC", "DISABLE", "ENABLE");
        ctx.collect_bitvec(tile, bel, "VRDSEL", "");

        let tile = "REG.COR2.S3A";
        ctx.collect_enum(
            tile,
            bel,
            "GWE_CYCLE",
            &["1", "2", "3", "4", "5", "6", "DONE", "KEEP"],
        );
        ctx.collect_enum(
            tile,
            bel,
            "GTS_CYCLE",
            &["1", "2", "3", "4", "5", "6", "DONE", "KEEP"],
        );
        ctx.collect_enum(tile, bel, "DONE_CYCLE", &["1", "2", "3", "4", "5", "6"]);
        ctx.collect_enum(
            tile,
            bel,
            "LCK_CYCLE",
            &["1", "2", "3", "4", "5", "6", "NOWAIT"],
        );
        ctx.collect_enum_bool(tile, "CAPTURE", "ONESHOT", "FALSE", "TRUE");
        ctx.collect_enum_bool(tile, bel, "BPI_DIV8", "NO", "YES");
        ctx.collect_enum_bool(tile, bel, "RESET_ON_ERR", "NO", "YES");
        ctx.collect_enum_bool(tile, "ICAP", "BYPASS", "NO", "YES");

        let tile = "REG.CTL.S3A";
        let bel = "MISC";
        ctx.collect_enum_bool(tile, bel, "GTS_USR_B", "NO", "YES");
        ctx.collect_enum_bool(tile, bel, "VGG_TEST", "NO", "YES");
        ctx.collect_enum_bool(tile, bel, "MULTIBOOT_ENABLE", "NO", "YES");
        ctx.collect_enum(
            tile,
            bel,
            "SECURITY",
            &["NONE", "LEVEL1", "LEVEL2", "LEVEL3"],
        );
        // too much trouble to deal with in normal ways.
        ctx.tiledb.insert(
            tile,
            bel,
            "PERSIST",
            TileItem::from_bit(TileBit::new(0, 0, 3), false),
        );
        ctx.collect_bit(tile, "ICAP", "ENABLE", "1");

        let tile = "REG.CCLK_FREQ";
        let bel = "STARTUP";
        let mut item = ctx.extract_enum_ocd(
            tile,
            bel,
            "CONFIG_RATE",
            &[
                "6", "1", "3", "7", "8", "10", "12", "13", "17", "22", "25", "27", "33", "44",
                "50", "100",
            ],
            OcdMode::BitOrder,
        );
        // a little fixup.
        assert_eq!(item.bits.len(), 9);
        assert_eq!(item.bits[8], TileBit::new(0, 0, 8));
        item.bits.push(TileBit::new(0, 0, 9));
        let TileItemKind::Enum { ref mut values } = item.kind else {
            unreachable!()
        };
        for val in values.values_mut() {
            val.push(false);
        }
        ctx.tiledb.insert(tile, bel, "CONFIG_RATE", item);
        ctx.collect_enum_int(tile, bel, "CCLK_DLY", 0..4, 0);
        ctx.collect_enum_int(tile, bel, "CCLK_SEP", 0..4, 0);
        ctx.collect_enum_int(tile, bel, "CLK_SWITCH_OPT", 0..4, 0);

        let tile = "REG.HC_OPT";
        let bel = "MISC";
        ctx.collect_enum_bool(tile, bel, "BRAM_SKIP", "NO", "YES");
        ctx.collect_enum_bool(tile, bel, "TWO_ROUND", "NO", "YES");
        ctx.collect_enum_int(tile, bel, "HC_CYCLE", 1..16, 0);

        let tile = "REG.POWERDOWN";
        let bel = "MISC";
        ctx.collect_enum(tile, bel, "SW_CLK", &["STARTUPCLK", "INTERNALCLK"]);
        ctx.collect_enum_bool(tile, bel, "EN_SUSPEND", "NO", "YES");
        ctx.collect_enum_bool(tile, bel, "EN_PORB", "NO", "YES");
        ctx.collect_enum_bool(tile, bel, "EN_SW_GSR", "NO", "YES");
        ctx.collect_enum_bool(tile, bel, "SUSPEND_FILTER", "NO", "YES");
        ctx.collect_enum_int(tile, bel, "WAKE_DELAY1", 1..8, 0);
        ctx.collect_enum_int(tile, bel, "WAKE_DELAY2", 1..32, 0);

        let tile = "REG.PU_GWE";
        ctx.collect_bitvec(tile, bel, "SW_GWE_CYCLE", "");

        let tile = "REG.PU_GTS";
        ctx.collect_bitvec(tile, bel, "SW_GTS_CYCLE", "");

        let tile = "REG.MODE";
        let bel = "MISC";
        ctx.collect_bitvec(tile, bel, "BOOTVSEL", "");
        ctx.collect_bitvec(tile, bel, "NEXT_CONFIG_BOOT_MODE", "");
        ctx.collect_enum_bool(tile, bel, "NEXT_CONFIG_NEW_MODE", "NO", "YES");
        ctx.collect_enum_bool(tile, bel, "TESTMODE_EN", "NO", "YES");

        let tile = "REG.GENERAL";
        ctx.collect_bitvec(tile, bel, "NEXT_CONFIG_ADDR", "");

        let tile = "REG.SEU_OPT";
        let bel = "MISC";
        ctx.collect_enum_bool(tile, bel, "GLUTMASK", "NO", "YES");
        ctx.collect_enum_bool(tile, bel, "POST_CRC_KEEP", "NO", "YES");

        // too much effort to include in the automatic fuzzer
        ctx.tiledb.insert(
            tile,
            bel,
            "POST_CRC_EN",
            TileItem::from_bit(TileBit::new(0, 0, 0), false),
        );

        let mut item = ctx.extract_enum_ocd(
            tile,
            bel,
            "POST_CRC_FREQ",
            &[
                "6", "1", "3", "7", "8", "10", "12", "13", "17", "22", "25", "27", "33", "44",
                "50", "100",
            ],
            OcdMode::BitOrder,
        );
        // a little fixup.
        assert_eq!(item.bits.len(), 9);
        assert_eq!(item.bits[8], TileBit::new(0, 0, 12));
        item.bits.push(TileBit::new(0, 0, 13));
        let TileItemKind::Enum { ref mut values } = item.kind else {
            unreachable!()
        };
        for val in values.values_mut() {
            val.push(false);
        }
        ctx.tiledb.insert(tile, bel, "POST_CRC_FREQ", item);

        // TODO
    }

    if edev.chip.kind.is_virtex2() {
        let is_double_grestore = ctx.empty_bs.die[DieId::from_idx(0)]
            .regs
            .get(&Reg::FakeDoubleGrestore)
            == Some(&1);
        ctx.insert_device_data("DOUBLE_GRESTORE", BitVec::repeat(is_double_grestore, 1));
    }
}
