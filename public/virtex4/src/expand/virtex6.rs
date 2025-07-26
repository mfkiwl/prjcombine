#![allow(clippy::comparison_chain)]

use prjcombine_interconnect::db::IntDb;
use prjcombine_interconnect::grid::{
    CellCoord, ColId, DieId, ExpandedGrid, Rect, RowId, TileIobId,
};
use prjcombine_xilinx_bitstream::{
    BitstreamGeom, DeviceKind, DieBitstreamGeom, FrameAddr, FrameInfo, FrameMaskMode,
};
use std::collections::{BTreeSet, HashSet};
use unnamed_entity::{EntityId, EntityPartVec, EntityVec};

use crate::bond::SharedCfgPad;
use crate::chip::{Chip, ColumnKind, DisabledPart, GtKind};
use crate::expanded::{DieFrameGeom, ExpandedDevice, IoCoord};
use crate::gtz::GtzDb;
use crate::regions;

struct Expander<'a, 'b> {
    chip: &'b Chip,
    disabled: &'a BTreeSet<DisabledPart>,
    egrid: &'a mut ExpandedGrid<'b>,
    die: DieId,
    site_holes: &'a mut Vec<Rect>,
    int_holes: &'a mut Vec<Rect>,
    hard_skip: HashSet<RowId>,
    frame_info: Vec<FrameInfo>,
    frames: DieFrameGeom,
    col_cfg: ColId,
    col_lio: Option<ColId>,
    col_rio: Option<ColId>,
    col_lcio: Option<ColId>,
    col_rcio: Option<ColId>,
    io: Vec<IoCoord>,
    gt: Vec<CellCoord>,
}

impl Expander<'_, '_> {
    fn is_site_hole(&self, cell: CellCoord) -> bool {
        for hole in &*self.site_holes {
            if hole.contains(cell) {
                return true;
            }
        }
        false
    }

    fn is_int_hole(&self, cell: CellCoord) -> bool {
        for hole in &*self.int_holes {
            if hole.contains(cell) {
                return true;
            }
        }
        false
    }

    fn fill_holes(&mut self) {
        let cfg_rect = CellCoord::new(
            self.die,
            self.col_cfg - 6,
            self.chip.row_reg_bot(self.chip.reg_cfg - 1),
        )
        .rect(6, 80);
        self.site_holes.push(cfg_rect);
        self.int_holes.push(cfg_rect);
        if let Some(ref hard) = self.chip.col_hard {
            for &row in &hard.rows_pcie {
                let cell = CellCoord::new(self.die, hard.col, row);
                self.site_holes.push(cell.delta(-3, 0).rect(4, 20));
                self.int_holes.push(cell.delta(-1, 0).rect(2, 20));
            }
            for &row in &hard.rows_emac {
                let cell = CellCoord::new(self.die, hard.col, row);
                self.site_holes.push(cell.rect(1, 10));
            }
        }
    }

    fn fill_int(&mut self) {
        for cell in self.egrid.die_cells(self.die) {
            if self.is_int_hole(cell) {
                continue;
            }
            self.egrid.add_tile_single(cell, "INT");
            if self.is_site_hole(cell) {
                continue;
            }
            match self.chip.columns[cell.col] {
                ColumnKind::ClbLL => (),
                ColumnKind::ClbLM => (),
                ColumnKind::Bram | ColumnKind::Dsp | ColumnKind::Io | ColumnKind::Cfg => {
                    self.egrid.add_tile_single(cell, "INTF");
                }
                ColumnKind::Gt => {
                    self.egrid.add_tile_single(cell, "INTF.DELAY");
                }
                _ => unreachable!(),
            }
        }
    }

    fn fill_cfg(&mut self) {
        let cell = CellCoord::new(
            self.die,
            self.col_cfg,
            self.chip.row_reg_bot(self.chip.reg_cfg),
        );
        for dx in 0..6 {
            if cell.row.to_idx() != 40 {
                self.egrid
                    .fill_conn_term(cell.delta(-6 + dx, -41), "TERM.N");
            }
            if cell.row.to_idx() != self.chip.regs * 40 - 40 {
                self.egrid.fill_conn_term(cell.delta(-6 + dx, 40), "TERM.S");
            }
        }
        self.egrid.add_tile_sn(cell, "CFG", 40, 80);
    }

    fn fill_terms(&mut self) {
        let row_b = self.chip.rows().next().unwrap();
        for cell in self.egrid.row(self.die, row_b) {
            if !self.is_int_hole(cell) {
                self.egrid.fill_conn_term(cell, "TERM.S.HOLE");
            }
        }
        let row_t = self.chip.rows().next_back().unwrap();
        for cell in self.egrid.row(self.die, row_t) {
            if !self.is_int_hole(cell) {
                self.egrid.fill_conn_term(cell, "TERM.N.HOLE");
            }
        }

        let col_l = self.chip.columns.ids().next().unwrap();
        for cell in self.egrid.column(self.die, col_l) {
            self.egrid.fill_conn_term(cell, "TERM.W");
        }
        let col_r = self.chip.columns.ids().next_back().unwrap();
        for cell in self.egrid.column(self.die, col_r) {
            self.egrid.fill_conn_term(cell, "TERM.E");
        }
    }

    fn fill_clb(&mut self) {
        for (col, &cd) in &self.chip.columns {
            let kind = match cd {
                ColumnKind::ClbLL => "CLBLL",
                ColumnKind::ClbLM => "CLBLM",
                _ => continue,
            };
            for cell in self.egrid.column(self.die, col) {
                if self.is_site_hole(cell) {
                    continue;
                }
                self.egrid.add_tile_single(cell, kind);
            }
        }
    }

    fn fill_hard(&mut self) {
        if let Some(ref hard) = self.chip.col_hard {
            for &row in &hard.rows_emac {
                let cell = CellCoord::new(self.die, hard.col, row);
                self.hard_skip.insert(row);
                self.hard_skip.insert(row + 5);
                let tcells = cell.cells_n_const::<10>();
                for cell in tcells {
                    self.egrid.add_tile_single(cell, "INTF.DELAY");
                }
                if self.disabled.contains(&DisabledPart::Emac(row)) {
                    continue;
                }
                self.egrid.add_tile(cell, "EMAC", &tcells);
            }

            for &row in &hard.rows_pcie {
                let cell = CellCoord::new(self.die, hard.col, row);
                if row.to_idx() != 0 {
                    self.egrid.fill_conn_term(cell.delta(-1, -1), "TERM.N");
                    self.egrid.fill_conn_term(cell.delta(0, -1), "TERM.N");
                }
                self.egrid.fill_conn_term(cell.delta(-1, 20), "TERM.S");
                self.egrid.fill_conn_term(cell.delta(0, 20), "TERM.S");

                for dy in [0, 5, 10, 15] {
                    self.hard_skip.insert(row + dy);
                }
                let mut tcells = vec![];
                tcells.extend(cell.delta(-3, 0).cells_n_const::<20>());
                tcells.extend(cell.delta(-2, 0).cells_n_const::<20>());
                for &cell in &tcells {
                    self.egrid.add_tile_single(cell, "INTF.DELAY");
                }
                self.egrid.add_tile(tcells[0], "PCIE", &tcells);
            }
        }
    }

    fn fill_bram_dsp(&mut self) {
        for (col, &cd) in &self.chip.columns {
            let kind = match cd {
                ColumnKind::Bram => "BRAM",
                ColumnKind::Dsp => "DSP",
                _ => continue,
            };
            for cell in self.egrid.column(self.die, col) {
                if !cell.row.to_idx().is_multiple_of(5) {
                    continue;
                }
                if let Some(ref hard) = self.chip.col_hard
                    && hard.col == col
                    && self.hard_skip.contains(&cell.row)
                {
                    continue;
                }
                self.egrid.add_tile_n(cell, kind, 5);
                if kind == "BRAM" && cell.row.to_idx() % 40 == 20 {
                    self.egrid.add_tile_n(cell, "PMVBRAM", 15);
                }
            }
        }
    }

    fn fill_io(&mut self) {
        for col in [self.col_lio, self.col_lcio, self.col_rcio, self.col_rio]
            .into_iter()
            .flatten()
        {
            for cell in self.egrid.column(self.die, col) {
                if cell.row.to_idx().is_multiple_of(2) {
                    self.egrid.add_tile_n(cell, "IO", 2);
                    self.io.extend([
                        IoCoord {
                            cell,
                            iob: TileIobId::from_idx(0),
                        },
                        IoCoord {
                            cell,
                            iob: TileIobId::from_idx(1),
                        },
                    ]);
                }

                if cell.row.to_idx() % 40 == 20 {
                    self.egrid.add_tile_sn(cell, "HCLK_IOI", 1, 2);
                }
            }
        }
    }

    fn fill_cmt(&mut self) {
        for reg in self.chip.regs() {
            let row_hclk = self.chip.row_reg_hclk(reg);
            let cell_hclk = CellCoord::new(self.die, self.col_cfg, row_hclk);
            self.egrid.add_tile_sn(cell_hclk, "CMT", 20, 40);

            let cell = cell_hclk.delta(0, -20);
            if reg < self.chip.reg_cfg - 1 {
                self.egrid.add_tile_n(cell, "PMVIOB", 2);
            } else if reg == self.chip.reg_cfg - 1 {
                // CMT_PMVB, empty
            } else if reg == self.chip.reg_cfg {
                self.egrid.add_tile_n(cell, "CMT_BUFG_TOP", 3);
            } else {
                self.egrid.add_tile(cell, "GCLK_BUF", &[]);
            }

            let cell = cell_hclk.delta(0, 18);
            if reg < self.chip.reg_cfg - 1 {
                self.egrid.add_tile(cell, "GCLK_BUF", &[]);
            } else if reg == self.chip.reg_cfg - 1 {
                self.egrid.add_tile_sn(cell, "CMT_BUFG_BOT", 1, 3);
            } else {
                self.egrid.add_tile_n(cell, "PMVIOB", 2);
            }
        }
    }

    fn fill_gt(&mut self) {
        for gtc in &self.chip.cols_gt {
            for reg in self.chip.regs() {
                if self.disabled.contains(&DisabledPart::GtxRow(reg)) {
                    continue;
                }
                let row_hclk = self.chip.row_reg_hclk(reg);
                let cell = CellCoord::new(self.die, gtc.col, row_hclk);
                let kind = gtc.regs[reg].unwrap();
                match kind {
                    GtKind::Gtx => {
                        self.egrid.add_tile_sn(cell, "GTX", 20, 40);
                    }
                    GtKind::Gth => {
                        self.egrid.add_tile_sn(cell, "GTH", 20, 40);
                    }
                    _ => unreachable!(),
                }
                self.gt.push(cell);
            }
        }
    }

    fn fill_hclk(&mut self) {
        for cell in self.egrid.die_cells(self.die) {
            let col_hrow = if cell.col <= self.col_cfg {
                self.col_cfg
            } else {
                self.col_cfg + 1
            };
            let row_hclk = self.chip.row_hclk(cell.row);
            let crow = if cell.row < row_hclk {
                row_hclk - 1
            } else {
                row_hclk
            };
            self.egrid[cell].region_root[regions::HCLK] = cell.with_cr(col_hrow, row_hclk);
            self.egrid[cell].region_root[regions::LEAF] = cell.with_row(crow);

            if cell.row.to_idx() % 40 == 20 {
                let skip_b = self.is_int_hole(cell.delta(0, -1));
                let skip_t = self.is_int_hole(cell);
                if skip_t && skip_b {
                    continue;
                }
                self.egrid.add_tile_sn(cell, "HCLK", 1, 2);
                if cell.col == self.chip.cols_qbuf.unwrap().0
                    || cell.col == self.chip.cols_qbuf.unwrap().1
                {
                    self.egrid.add_tile(cell, "HCLK_QBUF", &[]);
                }
                if self.chip.cols_mgt_buf.contains(&cell.col) {
                    self.egrid.add_tile(cell, "MGT_BUF", &[]);
                }
            }
        }
    }

    fn fill_frame_info(&mut self) {
        let mut regs: Vec<_> = self.chip.regs().collect();
        regs.sort_by_key(|&reg| {
            let rreg = reg - self.chip.reg_cfg;
            (rreg < 0, rreg.abs())
        });
        for _ in 0..self.chip.regs {
            self.frames.col_frame.push(EntityVec::new());
            self.frames.col_width.push(EntityVec::new());
            self.frames.bram_frame.push(EntityPartVec::new());
        }
        for &reg in &regs {
            for (col, &cd) in &self.chip.columns {
                self.frames.col_frame[reg].push(self.frame_info.len());
                let width = match cd {
                    ColumnKind::ClbLL => 36,
                    ColumnKind::ClbLM => 36,
                    ColumnKind::Bram => 28,
                    ColumnKind::Dsp => 28,
                    ColumnKind::Io => 44,
                    ColumnKind::Cfg => 38,
                    ColumnKind::Gt => 30,
                    _ => unreachable!(),
                };
                self.frames.col_width[reg].push(width as usize);
                for minor in 0..width {
                    let mut mask_mode = [FrameMaskMode::None; 2];
                    if cd == ColumnKind::Gt && matches!(minor, 28 | 29) {
                        mask_mode[0] = FrameMaskMode::DrpHclk(24, 13);
                        mask_mode[1] = FrameMaskMode::DrpHclk(25, 13);
                    }
                    if cd == ColumnKind::Cfg && matches!(minor, 26 | 27) {
                        mask_mode[0] = FrameMaskMode::CmtDrpHclk(24, 13);
                        mask_mode[1] = FrameMaskMode::CmtDrpHclk(25, 13);
                    }
                    if cd == ColumnKind::Cfg && matches!(minor, 34 | 35) && reg == self.chip.reg_cfg
                    {
                        mask_mode[0] = FrameMaskMode::DrpHclk(23, 13);
                        mask_mode[1] = FrameMaskMode::DrpHclk(23, 13);
                    }
                    if let Some(ref hard) = self.chip.col_hard
                        && col == hard.col
                        && hard.rows_pcie.contains(&self.chip.row_reg_bot(reg))
                        && matches!(minor, 26 | 27)
                    {
                        mask_mode[0] = FrameMaskMode::DrpHclk(24, 13);
                    }

                    self.frame_info.push(FrameInfo {
                        addr: FrameAddr {
                            typ: 0,
                            region: (reg - self.chip.reg_cfg) as i32,
                            major: col.to_idx() as u32,
                            minor,
                        },
                        mask_mode: mask_mode.into_iter().collect(),
                    });
                }
            }
        }
        for &reg in &regs {
            let mut major = 0;
            for (col, &cd) in &self.chip.columns {
                if cd != ColumnKind::Bram {
                    continue;
                }
                self.frames.bram_frame[reg].insert(col, self.frame_info.len());
                for minor in 0..128 {
                    self.frame_info.push(FrameInfo {
                        addr: FrameAddr {
                            typ: 1,
                            region: (reg - self.chip.reg_cfg) as i32,
                            major,
                            minor,
                        },
                        mask_mode: [FrameMaskMode::All; 2].into_iter().collect(),
                    });
                }
                major += 1;
            }
        }
    }
}

pub fn expand_grid<'a>(
    chips: &EntityVec<DieId, &'a Chip>,
    disabled: &BTreeSet<DisabledPart>,
    db: &'a IntDb,
    gdb: &'a GtzDb,
) -> ExpandedDevice<'a> {
    let mut egrid = ExpandedGrid::new(db);
    assert_eq!(chips.len(), 1);
    let chip = chips.first().unwrap();
    let die = egrid.add_die(chip.columns.len(), chip.regs * 40);

    let col_cfg = chip
        .columns
        .iter()
        .find_map(|(col, &cd)| {
            if cd == ColumnKind::Cfg {
                Some(col)
            } else {
                None
            }
        })
        .unwrap();
    let cols_lio: Vec<_> = chip
        .columns
        .iter()
        .filter_map(|(col, &cd)| {
            if cd == ColumnKind::Io && col < col_cfg {
                Some(col)
            } else {
                None
            }
        })
        .collect();
    let (col_lio, col_lcio) = match *cols_lio {
        [lc] => (None, Some(lc)),
        [l, lc] => (Some(l), Some(lc)),
        _ => unreachable!(),
    };
    let cols_rio: Vec<_> = chip
        .columns
        .iter()
        .filter_map(|(col, &cd)| {
            if cd == ColumnKind::Io && col > col_cfg {
                Some(col)
            } else {
                None
            }
        })
        .collect();
    let (col_rio, col_rcio) = match *cols_rio {
        [rc] => (None, Some(rc)),
        [rc, r] => (Some(r), Some(rc)),
        _ => unreachable!(),
    };
    let col_lgt = chip
        .cols_gt
        .iter()
        .find(|gtc| gtc.col < col_cfg)
        .map(|x| x.col);
    let col_rgt = chip
        .cols_gt
        .iter()
        .find(|gtc| gtc.col > col_cfg)
        .map(|x| x.col);

    let mut int_holes = vec![];
    let mut site_holes = vec![];

    let mut expander = Expander {
        chip,
        disabled,
        die,
        egrid: &mut egrid,
        int_holes: &mut int_holes,
        site_holes: &mut site_holes,
        hard_skip: HashSet::new(),
        frame_info: vec![],
        frames: DieFrameGeom {
            col_frame: EntityVec::new(),
            col_width: EntityVec::new(),
            bram_frame: EntityVec::new(),
            spine_frame: EntityVec::new(),
        },
        col_cfg,
        col_lio,
        col_rio,
        col_lcio,
        col_rcio,
        io: vec![],
        gt: vec![],
    };

    expander.fill_holes();
    expander.fill_int();
    expander.fill_cfg();
    expander.fill_hard();
    expander.fill_terms();
    expander.egrid.fill_main_passes(die);
    expander.fill_clb();
    expander.fill_bram_dsp();
    expander.fill_io();
    expander.fill_cmt();
    expander.fill_gt();
    expander.fill_hclk();
    expander.fill_frame_info();

    let frames = expander.frames;
    let io = expander.io;
    let gt = expander.gt;
    let die_bs_geom = DieBitstreamGeom {
        frame_len: 64 * 40 + 32,
        frame_info: expander.frame_info,
        bram_frame_len: 0,
        bram_frame_info: vec![],
        iob_frame_len: 0,
    };
    let bs_geom = BitstreamGeom {
        kind: DeviceKind::Virtex6,
        die: [die_bs_geom].into_iter().collect(),
        die_order: vec![die],
        has_gtz_bot: false,
        has_gtz_top: false,
    };

    let lcio = col_lcio.unwrap();
    let rcio = col_rcio.unwrap();
    let cfg_io = [
        (lcio, 6, 0, SharedCfgPad::CsoB),
        (lcio, 6, 1, SharedCfgPad::Rs(0)),
        (lcio, 8, 0, SharedCfgPad::Rs(1)),
        (lcio, 8, 1, SharedCfgPad::FweB),
        (lcio, 10, 0, SharedCfgPad::FoeB),
        (lcio, 10, 1, SharedCfgPad::FcsB),
        (lcio, 12, 0, SharedCfgPad::Data(0)),
        (lcio, 12, 1, SharedCfgPad::Data(1)),
        (lcio, 14, 0, SharedCfgPad::Data(2)),
        (lcio, 14, 1, SharedCfgPad::Data(3)),
        (lcio, 24, 0, SharedCfgPad::Data(4)),
        (lcio, 24, 1, SharedCfgPad::Data(5)),
        (lcio, 26, 0, SharedCfgPad::Data(6)),
        (lcio, 26, 1, SharedCfgPad::Data(7)),
        (lcio, 28, 0, SharedCfgPad::Data(8)),
        (lcio, 28, 1, SharedCfgPad::Data(9)),
        (lcio, 30, 0, SharedCfgPad::Data(10)),
        (lcio, 30, 1, SharedCfgPad::Data(11)),
        (lcio, 32, 0, SharedCfgPad::Data(12)),
        (lcio, 32, 1, SharedCfgPad::Data(13)),
        (lcio, 34, 0, SharedCfgPad::Data(14)),
        (lcio, 34, 1, SharedCfgPad::Data(15)),
        (rcio, 2, 0, SharedCfgPad::Addr(16)),
        (rcio, 2, 1, SharedCfgPad::Addr(17)),
        (rcio, 4, 0, SharedCfgPad::Addr(18)),
        (rcio, 4, 1, SharedCfgPad::Addr(19)),
        (rcio, 6, 0, SharedCfgPad::Addr(20)),
        (rcio, 6, 1, SharedCfgPad::Addr(21)),
        (rcio, 8, 0, SharedCfgPad::Addr(22)),
        (rcio, 8, 1, SharedCfgPad::Addr(23)),
        (rcio, 10, 0, SharedCfgPad::Addr(24)),
        (rcio, 10, 1, SharedCfgPad::Addr(25)),
        (rcio, 12, 0, SharedCfgPad::Data(16)),
        (rcio, 12, 1, SharedCfgPad::Data(17)),
        (rcio, 14, 0, SharedCfgPad::Data(18)),
        (rcio, 14, 1, SharedCfgPad::Data(19)),
        (rcio, 24, 0, SharedCfgPad::Data(20)),
        (rcio, 24, 1, SharedCfgPad::Data(21)),
        (rcio, 26, 0, SharedCfgPad::Data(22)),
        (rcio, 26, 1, SharedCfgPad::Data(23)),
        (rcio, 28, 0, SharedCfgPad::Data(24)),
        (rcio, 28, 1, SharedCfgPad::Data(25)),
        (rcio, 30, 0, SharedCfgPad::Data(26)),
        (rcio, 30, 1, SharedCfgPad::Data(27)),
        (rcio, 32, 0, SharedCfgPad::Data(28)),
        (rcio, 32, 1, SharedCfgPad::Data(29)),
        (rcio, 34, 0, SharedCfgPad::Data(30)),
        (rcio, 34, 1, SharedCfgPad::Data(31)),
    ]
    .into_iter()
    .map(|(col, dy, iob, pin)| {
        (
            pin,
            IoCoord {
                cell: CellCoord {
                    die: DieId::from_idx(0),
                    col,
                    row: chip.row_reg_bot(chip.reg_cfg) - 40 + dy,
                },
                iob: TileIobId::from_idx(iob),
            },
        )
    })
    .collect();

    egrid.finish();
    ExpandedDevice {
        kind: chip.kind,
        chips: chips.clone(),
        interposer: None,
        disabled: disabled.clone(),
        int_holes,
        site_holes,
        egrid,
        gdb,
        bs_geom,
        frames: [frames].into_iter().collect(),
        col_cfg,
        col_clk: col_cfg,
        col_lio,
        col_rio,
        col_lcio,
        col_rcio,
        col_lgt,
        col_rgt,
        col_mgt: None,
        row_dcmiob: None,
        row_iobdcm: None,
        io,
        gt,
        gtz: Default::default(),
        cfg_io,
        banklut: EntityVec::new(),
    }
}
