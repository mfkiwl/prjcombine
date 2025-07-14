use crate::db::{BelInfo, ConnectorWire, IntDb, IntfInfo, PinDir};
use std::collections::BTreeMap;
use unnamed_entity::EntityId;

impl IntDb {
    pub fn print(&self, o: &mut dyn std::io::Write) -> std::io::Result<()> {
        for (_, k, &w) in &self.wires {
            writeln!(o, "\tWIRE {k:14} {w}", w = w.to_string(self))?;
        }
        for slot in self.region_slots.values() {
            writeln!(o, "\tREGION SLOT {slot}")?;
        }
        for slot in self.tile_slots.values() {
            writeln!(o, "\tTILE SLOT {slot}")?;
        }
        for (_, name, bslot) in &self.bel_slots {
            writeln!(
                o,
                "\tBEL SLOT {name}: {tile_slot}",
                tile_slot = self.tile_slots[bslot.tile_slot]
            )?;
        }
        for (_, name, tcls) in &self.tile_classes {
            writeln!(
                o,
                "\tTILE CLASS {name} {slot} {nt}",
                slot = self.tile_slots[tcls.slot],
                nt = tcls.cells.len()
            )?;
            for (&wo, intf) in &tcls.intfs {
                match intf {
                    IntfInfo::OutputTestMux(ins) => {
                        write!(
                            o,
                            "\t\tINTF.TESTMUX {wot}.{won} <-",
                            wot = wo.cell.to_idx(),
                            won = self.wires.key(wo.wire)
                        )?;
                        for &wi in ins {
                            write!(
                                o,
                                " {wit}.{win}",
                                wit = wi.cell.to_idx(),
                                win = self.wires.key(wi.wire)
                            )?;
                        }
                        writeln!(o)?;
                    }
                    IntfInfo::OutputTestMuxPass(ins, wi) => {
                        write!(
                            o,
                            "\t\tINTF.TESTMUX.PASS {wot}.{won} <- {wit}.{win} | ",
                            wot = wo.cell.to_idx(),
                            won = self.wires.key(wo.wire),
                            wit = wi.cell.to_idx(),
                            win = self.wires.key(wi.wire)
                        )?;
                        for &wi in ins {
                            write!(
                                o,
                                " {wit}.{win}",
                                wit = wi.cell.to_idx(),
                                win = self.wires.key(wi.wire)
                            )?;
                        }
                        writeln!(o)?;
                    }
                    IntfInfo::InputDelay => {
                        writeln!(
                            o,
                            "\t\tINTF.DELAY {wot}.{won}",
                            wot = wo.cell.to_idx(),
                            won = self.wires.key(wo.wire)
                        )?;
                    }
                }
            }
            let mut wires: BTreeMap<_, Vec<_>> = BTreeMap::new();
            for (slot, bel) in &tcls.bels {
                match bel {
                    BelInfo::SwitchBox(sb) => {
                        writeln!(o, "\t\t{slot}: SWITCHBOX", slot = self.bel_slots.key(slot))?;
                        for item in &sb.items {
                            match item {
                                crate::db::SwitchBoxItem::Mux(mux) => {
                                    write!(
                                        o,
                                        "\t\t\tMUX      {dst:20} <- ",
                                        dst = mux.dst.to_string(self, tcls)
                                    )?;
                                    for src in &mux.src {
                                        write!(o, " {src:20}", src = src.to_string(self, tcls))?;
                                    }
                                    writeln!(o)?;
                                }
                                crate::db::SwitchBoxItem::ProgBuf(buf) => writeln!(
                                    o,
                                    "\t\t\tPROGBUF  {dst:20} <-  {src:20}",
                                    dst = buf.dst.to_string(self, tcls),
                                    src = buf.src.to_string(self, tcls),
                                )?,
                                crate::db::SwitchBoxItem::PermaBuf(buf) => writeln!(
                                    o,
                                    "\t\t\tPERMABUF {dst:20} <-  {src:20}",
                                    dst = buf.dst.to_string(self, tcls),
                                    src = buf.src.to_string(self, tcls),
                                )?,
                                crate::db::SwitchBoxItem::Pass(pass) => writeln!(
                                    o,
                                    "\t\t\tPASS     {dst:20} <-  {src:20}",
                                    dst = pass.dst.to_string(self, tcls),
                                    src = pass.src.to_string(self, tcls),
                                )?,
                                crate::db::SwitchBoxItem::BiPass(pass) => writeln!(
                                    o,
                                    "\t\t\tPASS     {a:20} <-> {b:20}",
                                    a = pass.a.to_string(self, tcls),
                                    b = pass.b.to_string(self, tcls),
                                )?,
                                crate::db::SwitchBoxItem::ProgInv(inv) => writeln!(
                                    o,
                                    "\t\t\tPROGINV  {dst:20} <-  {src:20}",
                                    dst = inv.dst.to_string(self, tcls),
                                    src = inv.src.to_string(self, tcls),
                                )?,
                                crate::db::SwitchBoxItem::ProgDelay(delay) => writeln!(
                                    o,
                                    "\t\t\tDELAY #{n} {dst:20} <-  {src:20}",
                                    n = delay.num_steps,
                                    dst = delay.dst.to_string(self, tcls),
                                    src = delay.src.to_string(self, tcls),
                                )?,
                            }
                        }
                    }
                    BelInfo::Bel(bel) => {
                        writeln!(o, "\t\t{slot}: BEL", slot = self.bel_slots.key(slot))?;
                        for (pn, pin) in &bel.pins {
                            write!(
                                o,
                                "\t\t\t{d}{intf} {pn:20}",
                                d = match pin.dir {
                                    PinDir::Input => " INPUT",
                                    PinDir::Output => "OUTPUT",
                                    PinDir::Inout => " INOUT",
                                },
                                intf = if pin.is_intf_in { ".INTF" } else { "     " }
                            )?;
                            for &wi in &pin.wires {
                                wires.entry(wi).or_default().push((slot, pn));
                                write!(o, " {wire}", wire = wi.to_string(self, tcls))?;
                            }
                            writeln!(o)?;
                        }
                    }
                }
            }
            for (wire, bels) in wires {
                write!(
                    o,
                    "\t\tWIRE {wt:3}.{wn:20}",
                    wt = wire.cell.to_idx(),
                    wn = self.wires.key(wire.wire)
                )?;
                for (bel, pin) in bels {
                    write!(o, " {bel}.{pin}", bel = self.bel_slots.key(bel))?;
                }
                writeln!(o)?;
            }
        }
        for (_, name, slot) in &self.conn_slots {
            writeln!(
                o,
                "\tCONN SLOT {name}: opposite {oname}",
                oname = self.conn_slots.key(slot.opposite)
            )?;
        }
        for (_, name, term) in &self.conn_classes {
            writeln!(
                o,
                "\tCONN CLASS {name} {slot}",
                slot = self.conn_slots.key(term.slot)
            )?;
            for (w, ti) in &term.wires {
                let wn = &self.wires.key(w);
                match ti {
                    ConnectorWire::BlackHole => {
                        writeln!(o, "\t\tBLACKHOLE {wn}")?;
                    }
                    &ConnectorWire::Reflect(ow) => {
                        writeln!(o, "\t\tPASS NEAR {wn} <- {own}", own = self.wires.key(ow))?;
                    }
                    &ConnectorWire::Pass(ow) => {
                        writeln!(o, "\t\tPASS FAR {wn} <- {own}", own = self.wires.key(ow))?;
                    }
                }
            }
        }
        Ok(())
    }
}
