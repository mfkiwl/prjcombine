use bitvec::prelude::*;
use prjcombine_re_fpga_hammer::{OcdMode, concat_bitvec, xlat_bit};
use prjcombine_re_hammer::Session;
use prjcombine_re_xilinx_geom::ExpandedDevice;
use prjcombine_spartan6::bels;
use prjcombine_types::tiledb::{TileBit, TileItem, TileItemKind};
use prjcombine_xilinx_bitstream::Reg;

use crate::{
    backend::{IseBackend, MultiValue},
    collector::CollectorCtx,
    generic::fbuild::{FuzzBuilderBase, FuzzCtx},
};

pub fn add_fuzzers<'a>(session: &mut Session<'a, IseBackend<'a>>, backend: &'a IseBackend<'a>) {
    let ExpandedDevice::Spartan6(edev) = backend.edev else {
        unreachable!()
    };
    for (tile, n) in [("LL", "BL"), ("UL", "TL"), ("LR", "BR"), ("UR", "TR")] {
        let mut ctx = FuzzCtx::new(session, backend, tile);
        for vh in ['V', 'H'] {
            ctx.build()
                .global("ENABLEMISR", "Y")
                .global("MISRRESET", "N")
                .test_manual("MISC", format!("MISR_{vh}_ENABLE"), "1")
                .global(format!("MISR_{n}{vh}_EN"), "Y")
                .commit();
            ctx.build()
                .global("ENABLEMISR", "Y")
                .global("MISRRESET", "Y")
                .test_manual("MISC", format!("MISR_{vh}_ENABLE_RESET"), "1")
                .global(format!("MISR_{n}{vh}_EN"), "Y")
                .commit();
        }
    }

    {
        let mut ctx = FuzzCtx::new(session, backend, "LL");
        ctx.build()
            .test_manual("MISC", "LEAKER_SLOPE_OPTIONS", "")
            .multi_global("LEAKERSLOPEOPTIONS", MultiValue::Dec(0), 4);
        ctx.build()
            .test_manual("MISC", "LEAKER_GAIN_OPTIONS", "")
            .multi_global("LEAKERGAINOPTIONS", MultiValue::Dec(0), 4);
        ctx.build()
            .test_manual("MISC", "VGG_SLOPE_OPTIONS", "")
            .multi_global("VGGSLOPEOPTIONS", MultiValue::Dec(0), 4);
        ctx.build()
            .test_manual("MISC", "VBG_SLOPE_OPTIONS", "")
            .multi_global("VBGSLOPEOPTIONS", MultiValue::Dec(0), 4);
        ctx.build()
            .test_manual("MISC", "VGG_TEST_OPTIONS", "")
            .multi_global("VGGTESTOPTIONS", MultiValue::Dec(0), 3);
        ctx.build()
            .test_manual("MISC", "VGG_COMP_OPTION", "")
            .multi_global("VGGCOMPOPTION", MultiValue::Dec(0), 1);
        for val in ["PULLUP", "PULLNONE"] {
            ctx.test_manual("MISC", "PROGPIN", val)
                .global("PROGPIN", val)
                .commit();
        }
        for val in ["PULLUP", "PULLNONE", "PULLDOWN"] {
            ctx.test_manual("MISC", "MISO2PIN", val)
                .global("MISO2PIN", val)
                .commit();
        }
        for bel in [bels::OCT_CAL2, bels::OCT_CAL3] {
            let mut bctx = ctx.bel(bel);
            bctx.test_manual("PRESENT", "1")
                .mode("OCT_CALIBRATE")
                .commit();
            bctx.mode("OCT_CALIBRATE")
                .test_enum("ACCESS_MODE", &["STATIC", "USER"]);
            bctx.mode("OCT_CALIBRATE")
                .test_enum("VREF_VALUE", &["0.25", "0.5", "0.75"]);
        }
    }

    {
        let mut ctx = FuzzCtx::new(session, backend, "UL");
        for val in ["READ", "PROGRAM"] {
            ctx.test_manual("MISC", "DNA_OPTIONS", val)
                .global("DNAOPTIONS", val)
                .commit();
        }
        ctx.test_manual("MISC", "DNA_OPTIONS", "ANALOG_READ")
            .global("DNAOPTIONS", "ANALOGREAD")
            .commit();
        for val in ["PULLUP", "PULLNONE", "PULLDOWN"] {
            for opt in ["M2PIN", "SELECTHSPIN"] {
                ctx.test_manual("MISC", opt, val).global(opt, val).commit();
            }
        }
        let mut bctx = ctx.bel(bels::DNA_PORT);
        bctx.test_manual("ENABLE", "1").mode("DNA_PORT").commit();
        let mut bctx = ctx.bel(bels::PMV);
        bctx.test_manual("PRESENT", "1").mode("PMV").commit();
        bctx.mode("PMV").test_multi_attr_dec("PSLEW", 4);
        bctx.mode("PMV").test_multi_attr_dec("NSLEW", 4);
        for bel in [bels::OCT_CAL0, bels::OCT_CAL4] {
            let mut bctx = ctx.bel(bel);
            bctx.test_manual("PRESENT", "1")
                .mode("OCT_CALIBRATE")
                .commit();
            bctx.mode("OCT_CALIBRATE")
                .test_enum("ACCESS_MODE", &["STATIC", "USER"]);
            bctx.mode("OCT_CALIBRATE")
                .test_enum("VREF_VALUE", &["0.25", "0.5", "0.75"]);
        }
    }

    {
        let mut ctx = FuzzCtx::new(session, backend, "LR");
        for val in ["PULLUP", "PULLNONE", "PULLDOWN"] {
            for opt in ["CCLK2PIN", "MOSI2PIN", "SS_BPIN"] {
                ctx.test_manual("MISC", opt, val).global(opt, val).commit();
            }
        }
        for val in ["PULLUP", "PULLNONE"] {
            ctx.test_manual("MISC", "DONEPIN", val)
                .global("DONEPIN", val)
                .commit();
        }
        let mut bctx = ctx.bel(bels::OCT_CAL1);
        bctx.test_manual("PRESENT", "1")
            .mode("OCT_CALIBRATE")
            .commit();
        bctx.mode("OCT_CALIBRATE")
            .test_enum("ACCESS_MODE", &["STATIC", "USER"]);
        bctx.mode("OCT_CALIBRATE")
            .test_enum("VREF_VALUE", &["0.25", "0.5", "0.75"]);
        let mut bctx = ctx.bel(bels::ICAP);
        bctx.test_manual("ENABLE", "1").mode("ICAP").commit();
        let mut bctx = ctx.bel(bels::SPI_ACCESS);
        bctx.test_manual("ENABLE", "1").mode("SPI_ACCESS").commit();

        let mut bctx = ctx.bel(bels::SUSPEND_SYNC);
        bctx.test_manual("ENABLE", "1")
            .mode("SUSPEND_SYNC")
            .commit();
        let mut bctx = ctx.bel(bels::POST_CRC_INTERNAL);
        bctx.test_manual("PRESENT", "1")
            .mode("POST_CRC_INTERNAL")
            .commit();
        let mut bctx = ctx.bel(bels::STARTUP);
        bctx.test_manual("PRESENT", "1").mode("STARTUP").commit();
        for attr in ["GTS_SYNC", "GSR_SYNC"] {
            for val in ["NO", "YES"] {
                bctx.mode("STARTUP")
                    .test_manual(attr, val)
                    .global(attr, val)
                    .commit();
            }
        }
        bctx.mode("STARTUP")
            .no_pin("GSR")
            .test_manual("PIN.GTS", "1")
            .pin("GTS")
            .commit();
        bctx.mode("STARTUP")
            .no_pin("GTS")
            .test_manual("PIN.GSR", "1")
            .pin("GSR")
            .commit();
        bctx.mode("STARTUP")
            .test_manual("PIN.CFGCLK", "1")
            .pin("CFGCLK")
            .commit();
        bctx.mode("STARTUP")
            .test_manual("PIN.CFGMCLK", "1")
            .pin("CFGMCLK")
            .commit();
        bctx.mode("STARTUP")
            .test_manual("PIN.KEYCLEARB", "1")
            .pin("KEYCLEARB")
            .commit();
        for val in ["CCLK", "USERCLK", "JTAGCLK"] {
            bctx.mode("STARTUP")
                .pin("CLK")
                .extra_tile_reg_attr(Reg::Cor1, "REG.COR1", "STARTUP", "STARTUPCLK", val)
                .null_bits()
                .test_manual("STARTUPCLK", val)
                .global("STARTUPCLK", val)
                .commit();
        }

        let mut bctx = ctx.bel(bels::SLAVE_SPI);
        bctx.test_manual("PRESENT", "1").mode("SLAVE_SPI").commit();
    }

    {
        let mut ctx = FuzzCtx::new(session, backend, "UR");
        for val in ["PULLUP", "PULLNONE", "PULLDOWN"] {
            for opt in ["TCKPIN", "TDIPIN", "TMSPIN", "TDOPIN", "CSO2PIN"] {
                ctx.test_manual("MISC", opt, val).global(opt, val).commit();
            }
        }
        let mut bctx = ctx.bel(bels::OCT_CAL5);
        bctx.test_manual("PRESENT", "1")
            .mode("OCT_CALIBRATE")
            .commit();
        bctx.mode("OCT_CALIBRATE")
            .test_enum("ACCESS_MODE", &["STATIC", "USER"]);
        bctx.mode("OCT_CALIBRATE")
            .test_enum("VREF_VALUE", &["0.25", "0.5", "0.75"]);
        for i in 0..4 {
            let mut bctx = ctx.bel(bels::BSCAN[i]);
            bctx.test_manual("ENABLE", "1").mode("BSCAN").commit();
            bctx.mode("BSCAN").test_enum("JTAG_TEST", &["0", "1"]);
        }
        ctx.test_manual("BSCAN_COMMON", "USERID", "").multi_global(
            "USERID",
            MultiValue::HexPrefix,
            32,
        );
    }

    let mut ctx = FuzzCtx::new_null(session, backend);
    {
        let reg = Reg::Cor1;
        let reg_name = "REG.COR1";
        // "STARTUP",
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
        for val in ["0", "1"] {
            for opt in ["SEND_VGG0", "SEND_VGG1", "SEND_VGG2", "SEND_VGG3"] {
                ctx.test_reg(reg, reg_name, "MISC", opt, val)
                    .global(opt, val)
                    .commit();
            }
        }
        for val in ["NO", "YES"] {
            for opt in ["VGG_SENDMAX", "VGG_ENABLE_OFFCHIP"] {
                ctx.test_reg(reg, reg_name, "MISC", opt, val)
                    .global(opt, val)
                    .commit();
            }
        }
    }

    {
        let reg = Reg::Cor2;
        let reg_name = "REG.COR2";
        for val in ["1", "2", "3", "4", "5", "6", "DONE", "KEEP"] {
            ctx.test_reg(reg, reg_name, "STARTUP", "GWE_CYCLE", val)
                .global("GWE_CYCLE", val)
                .commit();
            ctx.build()
                .global("LCK_CYCLE", "NOWAIT")
                .test_reg(reg, reg_name, "STARTUP", "GTS_CYCLE", val)
                .global("GTS_CYCLE", val)
                .commit();
        }
        for val in ["1", "2", "3", "4", "5", "6"] {
            ctx.test_reg(reg, reg_name, "STARTUP", "DONE_CYCLE", val)
                .global("DONE_CYCLE", val)
                .commit();
        }
        for val in ["1", "2", "3", "4", "5", "6", "NOWAIT"] {
            ctx.build()
                .global("GTS_CYCLE", "1")
                .test_reg(reg, reg_name, "STARTUP", "LCK_CYCLE", val)
                .global("LCK_CYCLE", val)
                .commit();
        }
        for val in ["NO", "YES"] {
            ctx.test_reg(reg, reg_name, "STARTUP", "BPI_DIV8", val)
                .global("BPI_DIV8", val)
                .commit();
            ctx.test_reg(reg, reg_name, "STARTUP", "BPI_DIV16", val)
                .global("BPI_DIV16", val)
                .commit();
            ctx.test_reg(reg, reg_name, "STARTUP", "RESET_ON_ERR", val)
                .global("RESET_ON_ERR", val)
                .commit();
            ctx.test_reg(reg, reg_name, "STARTUP", "DISABLE_VRD_REG", val)
                .global("DISABLE_VRD_REG", val)
                .commit();
        }
    }

    {
        let reg = Reg::Ctl0;
        let reg_name = "REG.CTL";
        for val in ["NO", "YES"] {
            ctx.test_reg(reg, reg_name, "MISC", "GTS_USR_B", val)
                .global("GTS_USR_B", val)
                .commit();
            ctx.test_reg(reg, reg_name, "MISC", "MULTIBOOT_ENABLE", val)
                .global("MULTIBOOTMODE", val)
                .commit();
            if edev.chip.has_encrypt {
                ctx.build()
                    .global_mutex("BRAM", "NOPE")
                    .test_reg(reg, reg_name, "MISC", "ENCRYPT", val)
                    .global("ENCRYPT", val)
                    .commit();
            }
        }
        for val in ["EFUSE", "BBRAM"] {
            ctx.test_reg(reg, reg_name, "MISC", "ENCRYPT_KEY_SELECT", val)
                .global("ENCRYPTKEYSELECT", val)
                .commit();
        }
        for val in ["DISABLE", "ENABLE"] {
            ctx.test_reg(reg, reg_name, "MISC", "POST_CRC_INIT_FLAG", val)
                .global("POST_CRC_INIT_FLAG", val)
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
        for val in ["2", "1", "4", "6", "10", "12", "16", "22", "26"] {
            ctx.build()
                .global("EXTMASTERCCLK_EN", "NO")
                .test_reg(reg, reg_name, "STARTUP", "CONFIG_RATE", val)
                .global("CONFIGRATE", val)
                .commit();
        }
        for val in ["1", "2", "4", "8", "16", "32", "64", "128", "256", "512"] {
            ctx.build()
                .global("EXTMASTERCCLK_EN", "YES")
                .test_reg(reg, reg_name, "STARTUP", "EXTMASTERCCLK_DIVIDE", val)
                .global("EXTMASTERCCLK_DIVIDE", val)
                .commit();
        }
        for val in ["NO", "YES"] {
            ctx.test_reg(reg, reg_name, "STARTUP", "EXTMASTERCCLK_EN", val)
                .global("EXTMASTERCCLK_EN", val)
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
            ctx.test_reg(reg, reg_name, "MISC", "SUSPEND_FILTER", val)
                .global("SUSPEND_FILTER", val)
                .commit();
            ctx.test_reg(reg, reg_name, "MISC", "EN_SW_GSR", val)
                .global("EN_SW_GSR", val)
                .commit();
            ctx.test_reg(reg, reg_name, "MISC", "MULTIPIN_WAKEUP", val)
                .global("MULTIPIN_WAKEUP", val)
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
        for val in ["1", "2", "4", "8", "16", "32", "64", "128", "256", "512"] {
            ctx.test_reg(reg, reg_name, "MISC", "SW_GWE_CYCLE", val)
                .global("SW_GWE_CYCLE", val)
                .commit();
        }
    }

    {
        let reg = Reg::PuGts;
        let reg_name = "REG.PU_GTS";
        for val in ["1", "2", "4", "8", "16", "32", "64", "128", "256", "512"] {
            ctx.test_reg(reg, reg_name, "MISC", "SW_GTS_CYCLE", val)
                .global("SW_GTS_CYCLE", val)
                .commit();
        }
    }

    {
        let reg = Reg::EyeMask;
        let reg_name = "REG.EYE_MASK";
        ctx.test_reg(reg, reg_name, "MISC", "WAKEUP_MASK", "")
            .multi_global("WAKEUP_MASK", MultiValue::HexPrefix, 8);
    }

    {
        let reg = Reg::Mode;
        let reg_name = "REG.MODE";
        for val in ["NO", "YES"] {
            ctx.test_reg(reg, reg_name, "MISC", "NEXT_CONFIG_NEW_MODE", val)
                .global("NEXT_CONFIG_NEW_MODE", val)
                .commit();
        }
        ctx.test_reg(reg, reg_name, "MISC", "NEXT_CONFIG_BOOT_MODE", "")
            .multi_global("NEXT_CONFIG_BOOT_MODE", MultiValue::Bin, 3);
    }

    {
        let reg = Reg::SeuOpt;
        let reg_name = "REG.SEU_OPT";
        for val in ["NO", "YES"] {
            ctx.test_reg(reg, reg_name, "MISC", "POST_CRC_KEEP", val)
                .global("POST_CRC_KEEP", val)
                .commit();
        }
        for val in ["0", "1"] {
            ctx.test_reg(reg, reg_name, "MISC", "POST_CRC_SEL", val)
                .global("POST_CRC_SEL", val)
                .commit();
            ctx.build()
                .global("POST_CRC_SEL", "0")
                .test_reg(reg, reg_name, "MISC", "POST_CRC_ONESHOT", val)
                .global("POST_CRC_ONESHOT", val)
                .commit();
        }
        for val in [
            "1", "2", "4", "6", "10", "12", "16", "22", "26", "33", "40", "50", "66",
        ] {
            ctx.test_reg(reg, reg_name, "MISC", "POST_CRC_FREQ", val)
                .global("POST_CRC_FREQ", val)
                .commit();
        }
    }

    {
        let reg = Reg::Testmode;
        let reg_name = "REG.TESTMODE";
        for val in ["NO", "YES"] {
            ctx.test_reg(reg, reg_name, "MISC", "TESTMODE_EN", val)
                .global("TESTMODE_EN", val)
                .commit();
            ctx.test_reg(reg, reg_name, "MISC", "ICAP_BYPASS", val)
                .global("ICAP_BYPASS", val)
                .commit();
            ctx.test_reg(reg, reg_name, "MISC", "VGG_TEST", val)
                .global("VGG_TEST", val)
                .commit();
        }
    }
}

pub fn collect_fuzzers(ctx: &mut CollectorCtx) {
    let ExpandedDevice::Spartan6(edev) = ctx.edev else {
        unreachable!()
    };
    for (tile, bel) in [
        ("LL", "OCT_CAL2"),
        ("LL", "OCT_CAL3"),
        ("UL", "OCT_CAL0"),
        ("UL", "OCT_CAL4"),
        ("LR", "OCT_CAL1"),
        ("UR", "OCT_CAL5"),
    ] {
        ctx.state.get_diff(tile, bel, "PRESENT", "1").assert_empty();
        ctx.state
            .get_diff(tile, bel, "VREF_VALUE", "0.25")
            .assert_empty();
        ctx.state
            .get_diff(tile, bel, "VREF_VALUE", "0.5")
            .assert_empty();
        ctx.state
            .get_diff(tile, bel, "VREF_VALUE", "0.75")
            .assert_empty();
        ctx.collect_enum(tile, bel, "ACCESS_MODE", &["STATIC", "USER"]);
    }

    {
        let tile = "LL";
        let bel = "MISC";
        ctx.collect_bitvec(tile, bel, "LEAKER_SLOPE_OPTIONS", "");
        ctx.collect_bitvec(tile, bel, "LEAKER_GAIN_OPTIONS", "");
        ctx.collect_bitvec(tile, bel, "VGG_SLOPE_OPTIONS", "");
        ctx.collect_bitvec(tile, bel, "VBG_SLOPE_OPTIONS", "");
        ctx.collect_bitvec(tile, bel, "VGG_TEST_OPTIONS", "");
        ctx.collect_bitvec(tile, bel, "VGG_COMP_OPTION", "");
        ctx.collect_enum(tile, bel, "PROGPIN", &["PULLUP", "PULLNONE"]);
        ctx.collect_enum(tile, bel, "MISO2PIN", &["PULLUP", "PULLNONE", "PULLDOWN"]);
    }
    for tile in ["LL", "UL", "LR", "UR"] {
        let bel = "MISC";
        ctx.collect_bit(tile, bel, "MISR_H_ENABLE", "1");
        ctx.collect_bit(tile, bel, "MISR_V_ENABLE", "1");
        let mut diff = ctx.state.get_diff(tile, bel, "MISR_H_ENABLE_RESET", "1");
        diff.apply_bit_diff(ctx.tiledb.item(tile, bel, "MISR_H_ENABLE"), true, false);
        ctx.tiledb.insert(tile, bel, "MISR_H_RESET", xlat_bit(diff));
        let mut diff = ctx.state.get_diff(tile, bel, "MISR_V_ENABLE_RESET", "1");
        diff.apply_bit_diff(ctx.tiledb.item(tile, bel, "MISR_V_ENABLE"), true, false);
        ctx.tiledb.insert(tile, bel, "MISR_V_RESET", xlat_bit(diff));
    }

    {
        let tile = "UL";
        let bel = "MISC";
        ctx.collect_enum(
            tile,
            bel,
            "DNA_OPTIONS",
            &["READ", "PROGRAM", "ANALOG_READ"],
        );
        ctx.collect_enum(tile, bel, "M2PIN", &["PULLUP", "PULLNONE", "PULLDOWN"]);
        ctx.collect_enum(
            tile,
            bel,
            "SELECTHSPIN",
            &["PULLUP", "PULLNONE", "PULLDOWN"],
        );
    }
    {
        let tile = "UL";
        let bel = "DNA_PORT";
        ctx.collect_bit(tile, bel, "ENABLE", "1");
    }
    {
        let tile = "UL";
        let bel = "PMV";
        ctx.state.get_diff(tile, bel, "PRESENT", "1").assert_empty();
        ctx.collect_bitvec(tile, bel, "PSLEW", "");
        ctx.collect_bitvec(tile, bel, "NSLEW", "");
    }

    {
        let tile = "LR";
        let bel = "MISC";
        ctx.collect_enum(tile, bel, "CCLK2PIN", &["PULLUP", "PULLNONE", "PULLDOWN"]);
        ctx.collect_enum(tile, bel, "MOSI2PIN", &["PULLUP", "PULLNONE", "PULLDOWN"]);
        ctx.collect_enum(tile, bel, "SS_BPIN", &["PULLUP", "PULLNONE", "PULLDOWN"]);
        ctx.collect_enum(tile, bel, "DONEPIN", &["PULLUP", "PULLNONE"]);
        ctx.collect_bit(tile, "ICAP", "ENABLE", "1");
        ctx.collect_bit(tile, "SUSPEND_SYNC", "ENABLE", "1");
        ctx.collect_bit(tile, "SPI_ACCESS", "ENABLE", "1");
        ctx.state
            .get_diff(tile, "SLAVE_SPI", "PRESENT", "1")
            .assert_empty();
        ctx.state
            .get_diff(tile, "POST_CRC_INTERNAL", "PRESENT", "1")
            .assert_empty();
    }
    {
        let tile = "LR";
        let bel = "STARTUP";
        ctx.state.get_diff(tile, bel, "PRESENT", "1").assert_empty();
        ctx.collect_enum_bool(tile, bel, "GTS_SYNC", "NO", "YES");
        ctx.collect_enum_bool(tile, bel, "GSR_SYNC", "NO", "YES");
        ctx.collect_bit(tile, bel, "PIN.CFGCLK", "1");
        ctx.collect_bit(tile, bel, "PIN.CFGMCLK", "1");
        ctx.collect_bit(tile, bel, "PIN.KEYCLEARB", "1");
        let item = ctx.extract_bit(tile, bel, "PIN.GTS", "1");
        ctx.tiledb.insert(tile, bel, "GTS_GSR_ENABLE", item);
        let item = ctx.extract_bit(tile, bel, "PIN.GSR", "1");
        ctx.tiledb.insert(tile, bel, "GTS_GSR_ENABLE", item);
    }

    {
        let tile = "UR";
        let bel = "MISC";
        ctx.collect_enum(tile, bel, "TCKPIN", &["PULLUP", "PULLNONE", "PULLDOWN"]);
        ctx.collect_enum(tile, bel, "TDIPIN", &["PULLUP", "PULLNONE", "PULLDOWN"]);
        ctx.collect_enum(tile, bel, "TMSPIN", &["PULLUP", "PULLNONE", "PULLDOWN"]);
        ctx.collect_enum(tile, bel, "TDOPIN", &["PULLUP", "PULLNONE", "PULLDOWN"]);
        ctx.collect_enum(tile, bel, "CSO2PIN", &["PULLUP", "PULLNONE", "PULLDOWN"]);
        ctx.collect_bit(tile, "BSCAN0", "ENABLE", "1");
        ctx.collect_bit(tile, "BSCAN1", "ENABLE", "1");
        ctx.collect_bit(tile, "BSCAN2", "ENABLE", "1");
        ctx.collect_bit(tile, "BSCAN3", "ENABLE", "1");
        ctx.collect_bitvec(tile, "BSCAN_COMMON", "USERID", "");
        let item = ctx.extract_enum_bool(tile, "BSCAN0", "JTAG_TEST", "0", "1");
        ctx.tiledb.insert(tile, "BSCAN_COMMON", "JTAG_TEST", item);
        for bel in ["BSCAN1", "BSCAN2", "BSCAN3"] {
            ctx.state
                .get_diff(tile, bel, "JTAG_TEST", "0")
                .assert_empty();
            ctx.state
                .get_diff(tile, bel, "JTAG_TEST", "1")
                .assert_empty();
        }
    }

    {
        let tile = "REG.COR1";
        let bel = "STARTUP";
        ctx.collect_enum(tile, bel, "STARTUPCLK", &["CCLK", "USERCLK", "JTAGCLK"]);
        ctx.collect_enum_bool(tile, bel, "DRIVE_DONE", "NO", "YES");
        ctx.collect_enum_bool(tile, bel, "DONE_PIPE", "NO", "YES");
        ctx.collect_enum_bool(tile, bel, "DRIVE_AWAKE", "NO", "YES");
        ctx.collect_enum_bool(tile, bel, "CRC", "DISABLE", "ENABLE");
        ctx.collect_bitvec(tile, bel, "VRDSEL", "");
        let bel = "MISC";
        ctx.collect_enum_bool(tile, bel, "VGG_SENDMAX", "NO", "YES");
        ctx.collect_enum_bool(tile, bel, "VGG_ENABLE_OFFCHIP", "NO", "YES");
        let item0 = ctx.extract_enum_bool(tile, bel, "SEND_VGG0", "0", "1");
        let item1 = ctx.extract_enum_bool(tile, bel, "SEND_VGG1", "0", "1");
        let item2 = ctx.extract_enum_bool(tile, bel, "SEND_VGG2", "0", "1");
        let item3 = ctx.extract_enum_bool(tile, bel, "SEND_VGG3", "0", "1");
        let item = concat_bitvec([item0, item1, item2, item3]);
        ctx.tiledb.insert(tile, bel, "SEND_VGG", item);

        let tile = "REG.COR2";
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
        ctx.collect_enum(tile, bel, "DONE_CYCLE", &["1", "2", "3", "4", "5", "6"]);
        ctx.collect_enum(
            tile,
            bel,
            "LCK_CYCLE",
            &["1", "2", "3", "4", "5", "6", "NOWAIT"],
        );
        ctx.collect_enum_bool(tile, bel, "BPI_DIV8", "NO", "YES");
        ctx.collect_enum_bool(tile, bel, "BPI_DIV16", "NO", "YES");
        ctx.collect_enum_bool(tile, bel, "RESET_ON_ERR", "NO", "YES");
        ctx.collect_enum_bool(tile, bel, "DISABLE_VRD_REG", "NO", "YES");
    }

    {
        let tile = "REG.CTL";
        let bel = "MISC";
        ctx.collect_enum_bool(tile, bel, "GTS_USR_B", "NO", "YES");
        ctx.collect_enum(tile, bel, "ENCRYPT_KEY_SELECT", &["BBRAM", "EFUSE"]);
        ctx.collect_enum_bool(tile, bel, "POST_CRC_INIT_FLAG", "DISABLE", "ENABLE");
        ctx.collect_enum_bool(tile, bel, "MULTIBOOT_ENABLE", "NO", "YES");
        if edev.chip.has_encrypt {
            ctx.collect_enum_bool(tile, bel, "ENCRYPT", "NO", "YES");
        }
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
    }

    {
        let tile = "REG.CCLK_FREQ";
        let bel = "STARTUP";
        // it's just 400 / val. boring.
        let _ = ctx.extract_enum_ocd(
            tile,
            bel,
            "CONFIG_RATE",
            &["2", "1", "4", "6", "10", "12", "16", "22", "26"],
            OcdMode::BitOrder,
        );
        let item =
            TileItem::from_bitvec((0..10).map(|bit| TileBit::new(0, 0, bit)).collect(), false);
        for i in 0..10 {
            let val = 1 << i;
            let mut diff = ctx
                .state
                .get_diff(tile, bel, "EXTMASTERCCLK_DIVIDE", val.to_string());
            diff.apply_bitvec_diff_int(&item, val, 1);
            diff.assert_empty();
        }
        ctx.state
            .get_diff(tile, bel, "EXTMASTERCCLK_EN", "NO")
            .assert_empty();
        let mut diff = ctx.state.get_diff(tile, bel, "EXTMASTERCCLK_EN", "YES");
        diff.apply_bitvec_diff_int(&item, 1, 0xc8);
        ctx.tiledb.insert(tile, bel, "CCLK_DIVISOR", item);
        ctx.tiledb
            .insert(tile, bel, "EXT_CCLK_ENABLE", xlat_bit(diff));
        ctx.collect_enum_int(tile, bel, "CCLK_DLY", 0..4, 0);
        ctx.collect_enum_int(tile, bel, "CCLK_SEP", 0..4, 0);
        for val in ["0", "1", "2", "3"] {
            ctx.state
                .get_diff(tile, bel, "CLK_SWITCH_OPT", val)
                .assert_empty();
        }
    }

    {
        let tile = "REG.HC_OPT";
        let bel = "MISC";
        ctx.collect_enum_bool(tile, bel, "BRAM_SKIP", "NO", "YES");
        ctx.collect_enum_bool(tile, bel, "TWO_ROUND", "NO", "YES");
        ctx.collect_enum_int(tile, bel, "HC_CYCLE", 1..16, 0);
        ctx.tiledb.insert(
            tile,
            bel,
            "INIT_SKIP",
            TileItem::from_bit(TileBit::new(0, 0, 6), false),
        );
    }

    {
        let tile = "REG.POWERDOWN";
        let bel = "MISC";
        ctx.collect_enum(tile, bel, "SW_CLK", &["STARTUPCLK", "INTERNALCLK"]);
        ctx.collect_enum_bool(tile, bel, "EN_SUSPEND", "NO", "YES");
        ctx.collect_enum_bool(tile, bel, "EN_SW_GSR", "NO", "YES");
        ctx.collect_enum_bool(tile, bel, "MULTIPIN_WAKEUP", "NO", "YES");
        ctx.collect_enum_bool(tile, bel, "SUSPEND_FILTER", "NO", "YES");
        ctx.collect_enum_int(tile, bel, "WAKE_DELAY1", 1..8, 0);
        ctx.collect_enum_int(tile, bel, "WAKE_DELAY2", 1..32, 0);
    }

    {
        let tile = "REG.PU_GWE";
        let bel = "MISC";
        let item =
            TileItem::from_bitvec((0..10).map(|bit| TileBit::new(0, 0, bit)).collect(), false);
        for i in 0..10 {
            let val = 1 << i;
            let mut diff = ctx
                .state
                .get_diff(tile, bel, "SW_GWE_CYCLE", val.to_string());
            diff.apply_bitvec_diff_int(&item, val, 5);
            diff.assert_empty();
        }
        ctx.tiledb.insert(tile, bel, "SW_GWE_CYCLE", item);
    }
    {
        let tile = "REG.PU_GTS";
        let bel = "MISC";
        let item =
            TileItem::from_bitvec((0..10).map(|bit| TileBit::new(0, 0, bit)).collect(), false);
        for i in 0..10 {
            let val = 1 << i;
            let mut diff = ctx
                .state
                .get_diff(tile, bel, "SW_GTS_CYCLE", val.to_string());
            diff.apply_bitvec_diff_int(&item, val, 4);
            diff.assert_empty();
        }
        ctx.tiledb.insert(tile, bel, "SW_GTS_CYCLE", item);
    }

    {
        let tile = "REG.EYE_MASK";
        let bel = "MISC";
        ctx.collect_bitvec(tile, bel, "WAKEUP_MASK", "");
    }

    {
        let tile = "REG.MODE";
        let bel = "MISC";
        ctx.collect_enum_bool(tile, bel, "NEXT_CONFIG_NEW_MODE", "NO", "YES");
        ctx.tiledb.insert(
            tile,
            bel,
            "SPI_BUSWIDTH",
            TileItem {
                bits: vec![TileBit::new(0, 0, 11), TileBit::new(0, 0, 12)],
                kind: TileItemKind::Enum {
                    values: [
                        ("1".to_string(), bitvec![0, 0]),
                        ("2".to_string(), bitvec![1, 0]),
                        ("4".to_string(), bitvec![0, 1]),
                    ]
                    .into_iter()
                    .collect(),
                },
            },
        );
        ctx.collect_bitvec(tile, bel, "NEXT_CONFIG_BOOT_MODE", "");
    }

    // these have annoying requirements to fuzz.
    ctx.tiledb.insert(
        "REG.GENERAL12",
        "MISC",
        "NEXT_CONFIG_ADDR",
        TileItem::from_bitvec(
            (0..16)
                .map(|bit| TileBit::new(0, 0, bit))
                .chain((0..16).map(|bit| TileBit::new(1, 0, bit)))
                .collect(),
            false,
        ),
    );
    ctx.tiledb.insert(
        "REG.GENERAL34",
        "MISC",
        "GOLDEN_CONFIG_ADDR",
        TileItem::from_bitvec(
            (0..16)
                .map(|bit| TileBit::new(0, 0, bit))
                .chain((0..16).map(|bit| TileBit::new(1, 0, bit)))
                .collect(),
            false,
        ),
    );
    ctx.tiledb.insert(
        "REG.GENERAL5",
        "MISC",
        "FAILSAFE_USER",
        TileItem::from_bitvec((0..16).map(|bit| TileBit::new(0, 0, bit)).collect(), false),
    );
    ctx.tiledb.insert(
        "REG.TIMER",
        "MISC",
        "TIMER_CFG",
        TileItem::from_bitvec((0..16).map(|bit| TileBit::new(0, 0, bit)).collect(), false),
    );

    {
        let tile = "REG.SEU_OPT";
        let bel = "MISC";
        ctx.collect_enum_bool(tile, bel, "POST_CRC_KEEP", "NO", "YES");
        ctx.collect_enum_bool(tile, bel, "POST_CRC_ONESHOT", "0", "1");
        ctx.collect_enum_bool(tile, bel, "POST_CRC_SEL", "0", "1");

        // too much effort to include in the automatic fuzzer
        ctx.tiledb.insert(
            tile,
            bel,
            "POST_CRC_EN",
            TileItem::from_bit(TileBit::new(0, 0, 0), false),
        );
        ctx.tiledb.insert(
            tile,
            bel,
            "GLUTMASK",
            TileItem::from_bit(TileBit::new(0, 0, 1), false),
        );

        // again, don't care.
        let _ = ctx.extract_enum_ocd(
            tile,
            bel,
            "POST_CRC_FREQ",
            &[
                "1", "2", "4", "6", "10", "12", "16", "22", "26", "33", "40", "50", "66",
            ],
            OcdMode::BitOrder,
        );
        ctx.tiledb.insert(
            tile,
            bel,
            "POST_CRC_FREQ",
            TileItem::from_bitvec((4..14).map(|bit| TileBit::new(0, 0, bit)).collect(), false),
        );
    }

    {
        let tile = "REG.TESTMODE";
        let bel = "MISC";
        ctx.collect_enum_bool(tile, bel, "VGG_TEST", "NO", "YES");
        ctx.collect_enum_bool(tile, bel, "ICAP_BYPASS", "NO", "YES");
        ctx.collect_enum_bool(tile, bel, "TESTMODE_EN", "NO", "YES");
    }
}
