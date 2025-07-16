use prjcombine_interconnect::grid::{ColId, DieId, RowId};
use prjcombine_ultrascale::chip::{Chip, CleMKind, ColumnKind, HardRowKind, IoRowKind};
use prjcombine_ultrascale::expanded::ExpandedDevice;
use unnamed_entity::{EntityId, EntityVec};

use crate::drawer::Drawer;

const W_CLB: f64 = 10.;
const W_DSP: f64 = 16.;
const W_BRAM: f64 = 40.;
const W_URAM: f64 = 120.;
const W_HARD: f64 = 120.;
const W_HDIOS: f64 = 160.;
const W_IO: f64 = 240.;
const W_TERM: f64 = 4.;
const W_BRK: f64 = 2.;
const H_TERM: f64 = 4.;
const H_CLB: f64 = 8.;
const H_HCLK: f64 = 2.;
const H_BRKH: f64 = 2.;
const H_HBM: f64 = 40.;

pub fn draw_device(name: &str, edev: ExpandedDevice) -> Drawer {
    let mut x = 0.;
    let mut col_x = EntityVec::new();
    let pgrid = edev.chips.first().unwrap();
    x += W_TERM;
    for (col, &cd) in &pgrid.columns {
        if pgrid.cols_vbrk.contains(&col) {
            x += W_BRK;
        }
        let xl = x;
        let w = match cd.kind {
            ColumnKind::CleL(_) | ColumnKind::CleM(_) => W_CLB,
            ColumnKind::Bram(_) => W_BRAM,
            ColumnKind::Dsp(_) => W_DSP,
            ColumnKind::Io(_) | ColumnKind::Gt(_) => W_IO,
            ColumnKind::Uram => W_URAM,
            ColumnKind::Hard(_, _)
            | ColumnKind::DfeB
            | ColumnKind::DfeC
            | ColumnKind::DfeDF
            | ColumnKind::DfeE
            | ColumnKind::Sdfec => W_HARD,
            ColumnKind::HdioS => W_HDIOS,
            ColumnKind::ContUram | ColumnKind::ContHard => 0.0,
        };
        x += w;
        col_x.push((xl, x));
    }
    x += W_TERM;
    let width = x;

    let mut y = 0.;
    let mut die_y: EntityVec<DieId, _> = EntityVec::new();
    let mut row_y = EntityVec::new();
    for (_, grid) in &edev.chips {
        let term_y_b = y;
        let mut die_row_y = EntityVec::new();
        y += H_TERM;
        if grid.has_hbm {
            y += H_HBM;
        }
        for row in grid.rows() {
            if row.to_idx().is_multiple_of(Chip::ROWS_PER_REG) {
                y += H_BRKH;
            }
            if row.to_idx() % Chip::ROWS_PER_REG == Chip::ROWS_PER_REG / 2 {
                y += H_HCLK;
            }
            die_row_y.push((y, y + H_CLB));
            y += H_CLB;
        }
        y += H_TERM;
        row_y.push(die_row_y);
        die_y.push((term_y_b, y));
    }
    let height = y;
    let mut drawer = Drawer::new(name.to_string(), width, height);
    drawer.bel_class("clel", "#00cc00");
    drawer.bel_class("clem", "#00ff00");
    drawer.bel_class("laguna", "#ff80ff");
    drawer.bel_class("bram", "#5555ff");
    drawer.bel_class("uram", "#0000ff");
    drawer.bel_class("dsp", "#00aaaa");
    drawer.bel_class("hrio", "#ff33ff");
    drawer.bel_class("hdio", "#ff66ff");
    drawer.bel_class("hdiolc", "#ff55ff");
    drawer.bel_class("hpio", "#ff00ff");
    drawer.bel_class("xp5io", "#ee00ee");
    drawer.bel_class("gth", "#c000ff");
    drawer.bel_class("gty", "#8000ff");
    drawer.bel_class("gtm", "#4000ff");
    drawer.bel_class("gtf", "#4000c0");
    drawer.bel_class("sysmon", "#aa00aa");
    drawer.bel_class("hsdac", "#4040c0");
    drawer.bel_class("hsadc", "#8040c0");
    drawer.bel_class("rfdac", "#2020c0");
    drawer.bel_class("rfadc", "#4020c0");
    drawer.bel_class("ps", "#ff0000");
    drawer.bel_class("vcu", "#aa0000");
    drawer.bel_class("hbm", "#aa0000");
    drawer.bel_class("cfg", "#ff8000");
    drawer.bel_class("pcie", "#ff0000");
    drawer.bel_class("ilkn", "#aa0000");
    drawer.bel_class("cmac", "#ff3333");
    drawer.bel_class("dfea", "#aa0055");
    drawer.bel_class("dfeb", "#aa3300");
    drawer.bel_class("dfec", "#aa3355");
    drawer.bel_class("dfed", "#ff0055");
    drawer.bel_class("dfee", "#ff3300");
    drawer.bel_class("dfef", "#ff3355");
    drawer.bel_class("dfeg", "#cc0033");
    drawer.bel_class("sdfec", "#cc3333");

    for (die, grid) in &edev.chips {
        for (col, &cd) in &grid.columns {
            match cd.kind {
                ColumnKind::CleL(_) | ColumnKind::CleM(_) => {
                    for row in grid.rows() {
                        let kind = match cd.kind {
                            ColumnKind::CleL(_) => "clel",
                            ColumnKind::CleM(CleMKind::Laguna) if grid.is_laguna_row(row) => {
                                "laguna"
                            }
                            _ => "clem",
                        };
                        if edev.in_site_hole(die, col, row) {
                            continue;
                        }
                        drawer.bel_rect(
                            col_x[col].0,
                            col_x[col].1,
                            row_y[die][row].0,
                            row_y[die][row].1,
                            kind,
                        )
                    }
                }
                ColumnKind::Bram(_) => {
                    for row in grid.rows().step_by(5) {
                        if edev.in_site_hole(die, col, row) {
                            continue;
                        }
                        drawer.bel_rect(
                            col_x[col].0,
                            col_x[col].1,
                            row_y[die][row].0,
                            row_y[die][row + 4].1,
                            "bram",
                        )
                    }
                }
                ColumnKind::Dsp(_) => {
                    for row in grid.rows().step_by(5) {
                        if edev.in_site_hole(die, col, row) {
                            continue;
                        }
                        drawer.bel_rect(
                            col_x[col].0,
                            col_x[col].1,
                            row_y[die][row].0,
                            row_y[die][row + 4].1,
                            "dsp",
                        )
                    }
                }
                ColumnKind::Uram => {
                    for row in grid.rows().step_by(15) {
                        if edev.in_site_hole(die, col, row) {
                            continue;
                        }
                        drawer.bel_rect(
                            col_x[col].0,
                            col_x[col].1,
                            row_y[die][row].0,
                            row_y[die][row + 14].1,
                            "uram",
                        )
                    }
                }
                ColumnKind::Io(idx) | ColumnKind::Gt(idx) => {
                    for (reg, kind) in &grid.cols_io[idx].regs {
                        let kind = match kind {
                            IoRowKind::None => continue,
                            IoRowKind::Hpio => "hpio",
                            IoRowKind::Hrio => "hrio",
                            IoRowKind::HdioL => "hdiolc",
                            IoRowKind::Xp5io => "xp5io",
                            IoRowKind::Gth => "gth",
                            IoRowKind::Gty => "gty",
                            IoRowKind::Gtm => "gtm",
                            IoRowKind::Gtf => "gtf",
                            IoRowKind::HsAdc => "hsadc",
                            IoRowKind::HsDac => "hsdac",
                            IoRowKind::RfAdc => "rfadc",
                            IoRowKind::RfDac => "rfdac",
                        };
                        drawer.bel_rect(
                            col_x[col].0,
                            col_x[col].1,
                            row_y[die][grid.row_reg_bot(reg)].0,
                            row_y[die][grid.row_reg_bot(reg + 1) - 1].1,
                            kind,
                        )
                    }
                }
                ColumnKind::HdioS => {
                    for reg in grid.regs() {
                        drawer.bel_rect(
                            col_x[col].0,
                            col_x[col].1,
                            row_y[die][grid.row_reg_bot(reg)].0,
                            row_y[die][grid.row_reg_bot(reg + 1) - 1].1,
                            "hdiolc",
                        )
                    }
                }
                ColumnKind::Sdfec => {
                    for reg in grid.regs() {
                        drawer.bel_rect(
                            col_x[col].0,
                            col_x[col].1,
                            row_y[die][grid.row_reg_bot(reg)].0,
                            row_y[die][grid.row_reg_bot(reg + 1) - 1].1,
                            "sdfec",
                        )
                    }
                }
                ColumnKind::Hard(_, idx) => {
                    for (reg, kind) in &grid.cols_hard[idx].regs {
                        let kind = match kind {
                            HardRowKind::Cfg => "cfg",
                            HardRowKind::Ams => "sysmon",
                            HardRowKind::None => continue,
                            HardRowKind::Hdio | HardRowKind::HdioAms => "hdio",
                            HardRowKind::HdioL => "hdiolc",
                            HardRowKind::Pcie | HardRowKind::Pcie4C | HardRowKind::Pcie4CE => {
                                "pcie"
                            }
                            HardRowKind::Cmac => "cmac",
                            HardRowKind::Ilkn => "ilkn",
                            HardRowKind::DfeA => "dfea",
                            HardRowKind::DfeG => "dfeg",
                        };
                        drawer.bel_rect(
                            col_x[col].0,
                            col_x[col].1,
                            row_y[die][grid.row_reg_bot(reg)].0,
                            row_y[die][grid.row_reg_bot(reg + 1) - 1].1,
                            kind,
                        )
                    }
                }
                ColumnKind::DfeB | ColumnKind::DfeC | ColumnKind::DfeDF | ColumnKind::DfeE => {
                    for reg in grid.regs() {
                        let kind = match cd.kind {
                            ColumnKind::DfeB => "dfeb",
                            ColumnKind::DfeC => "dfec",
                            ColumnKind::DfeDF => {
                                if reg.to_idx() == 2 {
                                    "dfef"
                                } else {
                                    "dfed"
                                }
                            }
                            ColumnKind::DfeE => "dfee",
                            _ => unreachable!(),
                        };
                        drawer.bel_rect(
                            col_x[col].0,
                            col_x[col].1,
                            row_y[die][grid.row_reg_bot(reg)].0,
                            row_y[die][grid.row_reg_bot(reg + 1) - 1].1,
                            kind,
                        )
                    }
                }
                ColumnKind::ContUram | ColumnKind::ContHard => (),
            }
        }
        if let Some(ps) = grid.ps {
            let col_l = ColId::from_idx(0);
            let row_b = if ps.has_vcu {
                let row_t = RowId::from_idx(Chip::ROWS_PER_REG);
                drawer.bel_rect(
                    col_x[col_l].0,
                    col_x[ps.col].1,
                    row_y[die][RowId::from_idx(0)].0,
                    row_y[die][row_t - 1].1,
                    "vcu",
                );
                row_t
            } else {
                RowId::from_idx(0)
            };
            drawer.bel_rect(
                col_x[col_l].0,
                col_x[ps.col].1,
                row_y[die][row_b].0,
                row_y[die][row_b + 3 * Chip::ROWS_PER_REG - 1].1,
                "ps",
            )
        }
        if grid.has_hbm {
            let col_l = grid.columns.first_id().unwrap();
            let col_r = grid.columns.last_id().unwrap();
            let row_b = RowId::from_idx(0);
            let mut points = vec![
                (col_x[col_l].0, row_y[die][row_b].0 - H_HBM),
                (col_x[col_l].0, row_y[die][row_b].0),
            ];
            for (col, cd) in &grid.columns {
                if matches!(cd.kind, ColumnKind::Dsp(_)) {
                    points.extend([
                        (col_x[col].0, row_y[die][row_b].0),
                        (col_x[col].0, row_y[die][row_b + 14].1),
                        (col_x[col].1, row_y[die][row_b + 14].1),
                        (col_x[col].1, row_y[die][row_b].0),
                    ]);
                }
            }
            points.extend([
                (col_x[col_r].1, row_y[die][row_b].0),
                (col_x[col_r].1, row_y[die][row_b].0 - H_HBM),
            ]);
            drawer.bel_poly(points, "hbm");
        }
    }
    drawer
}
