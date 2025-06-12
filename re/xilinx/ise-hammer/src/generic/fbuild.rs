use prjcombine_interconnect::{
    db::{BelSlotId, TileClassId},
    dir::DirV,
    grid::TileCoord,
};
use prjcombine_re_fpga_hammer::{FeatureId, FpgaFuzzerGen, FuzzerProp};
use prjcombine_re_hammer::Session;
use prjcombine_xilinx_bitstream::Reg;

use crate::backend::{IseBackend, Key, MultiValue, PinFromKind, Value};

use super::props::{
    BaseRaw, DynProp, FuzzRaw, FuzzRawMulti, NullBits,
    bel::{
        BaseBelAttr, BaseBelMode, BaseBelNoPin, BaseBelPin, BaseBelPinFrom, BaseBelPinPips,
        BaseGlobalXy, BelMutex, ForceBelName, FuzzBelAttr, FuzzBelMode, FuzzBelMultiAttr,
        FuzzBelPin, FuzzBelPinFrom, FuzzBelPinIntPips, FuzzBelPinPair, FuzzBelPinPips,
        FuzzGlobalXy, FuzzMultiGlobalXy, GlobalMutexHere, RowMutexHere,
    },
    extra::{ExtraGtz, ExtraReg, ExtraTile, ExtraTilesByBel, ExtraTilesByKind},
    mutex::{IntMutex, RowMutex, TileMutex, TileMutexExclusive},
    pip::{BasePip, BelIntoPipWire, FuzzPip},
    relation::{FixedRelation, HasRelated, NoopRelation, Related, TileRelation},
};

pub struct FuzzCtx<'sm, 'a> {
    pub session: &'sm mut Session<'a, IseBackend<'a>>,
    pub backend: &'a IseBackend<'a>,
    pub node_kind: Option<TileClassId>,
}

impl<'sm, 'b> FuzzCtx<'sm, 'b> {
    pub fn new(
        session: &'sm mut Session<'b, IseBackend<'b>>,
        backend: &'b IseBackend<'b>,
        tile: impl Into<String>,
    ) -> Self {
        let tile = tile.into();
        let node_kind = backend.egrid.db.get_tile_class(&tile);
        Self {
            session,
            backend,
            node_kind: Some(node_kind),
        }
    }

    pub fn try_new(
        session: &'sm mut Session<'b, IseBackend<'b>>,
        backend: &'b IseBackend<'b>,
        tile: impl Into<String>,
    ) -> Option<Self> {
        let tile = tile.into();
        let node_kind = backend.egrid.db.get_tile_class(&tile);
        if backend.egrid.tile_index[node_kind].is_empty() {
            return None;
        }
        Some(Self {
            session,
            backend,
            node_kind: Some(node_kind),
        })
    }

    pub fn new_null(
        session: &'sm mut Session<'b, IseBackend<'b>>,
        backend: &'b IseBackend<'b>,
    ) -> Self {
        Self {
            session,
            backend,
            node_kind: None,
        }
    }

    pub fn bel<'c>(&'c mut self, bel: BelSlotId) -> FuzzCtxBel<'c, 'b> {
        FuzzCtxBel {
            session: &mut *self.session,
            backend: self.backend,
            node_kind: self.node_kind.unwrap(),
            bel,
        }
    }

    pub fn test_manual<'nsm>(
        &'nsm mut self,
        bel: &'static str,
        attr: impl AsRef<str>,
        val: impl AsRef<str>,
    ) -> FuzzBuilderTestManual<'nsm, 'b> {
        self.build().test_manual(bel, attr, val)
    }

    pub fn test_reg<'nsm>(
        &'nsm mut self,
        reg: Reg,
        tile: impl Into<String>,
        bel: &'static str,
        attr: impl AsRef<str>,
        val: impl AsRef<str>,
    ) -> FuzzBuilderTestManual<'nsm, 'b> {
        self.build().test_reg(reg, tile, bel, attr, val)
    }

    pub fn build<'nsm>(&'nsm mut self) -> FuzzBuilder<'nsm, 'b> {
        FuzzBuilder {
            session: &mut *self.session,
            backend: self.backend,
            node_kind: self.node_kind,
            props: vec![],
        }
    }
}

pub trait FuzzBuilderBase<'b>: Sized {
    fn prop_box(self, prop: Box<DynProp<'b>>) -> Self;
    fn backend(&self) -> &'b IseBackend<'b>;

    fn prop(self, prop: impl FuzzerProp<'b, IseBackend<'b>> + 'b) -> Self {
        self.prop_box(Box::new(prop))
    }

    fn raw(self, key: Key<'b>, val: impl Into<Value<'b>>) -> Self {
        self.prop(BaseRaw::new(key, val.into()))
    }

    fn raw_diff(
        self,
        key: Key<'b>,
        val0: impl Into<Value<'b>>,
        val1: impl Into<Value<'b>>,
    ) -> Self {
        self.prop(FuzzRaw::new(key, val0.into(), val1.into()))
    }

    fn global(self, opt: impl Into<String>, val: impl Into<String>) -> Self {
        self.raw(Key::GlobalOpt(opt.into()), val.into())
    }

    fn no_global(self, opt: impl Into<String>) -> Self {
        self.raw(Key::GlobalOpt(opt.into()), None)
    }

    fn global_mutex(self, key: impl Into<String>, val: impl Into<String>) -> Self {
        self.raw(Key::GlobalMutex(key.into()), val.into())
    }

    fn row_mutex(self, key: impl Into<String>, val: impl Into<String>) -> Self {
        let prop = RowMutex::new(key.into(), val.into());
        self.prop(prop)
    }

    fn tile_mutex(self, key: impl Into<String>, val: impl Into<String>) -> Self {
        let prop = TileMutex::new(key.into(), val.into());
        self.prop(prop)
    }

    fn related_tile_mutex<R: TileRelation + 'b>(
        self,
        relation: R,
        key: impl Into<String>,
        val: impl Into<String>,
    ) -> Self {
        let prop = Related::new(relation, TileMutex::new(key.into(), val.into()));
        self.prop(prop)
    }

    fn related_tile_mutex_exclusive<R: TileRelation + 'b>(
        self,
        relation: R,
        key: impl Into<String>,
    ) -> Self {
        let prop = Related::new(relation, TileMutexExclusive::new(key.into()));
        self.prop(prop)
    }

    fn maybe_prop(self, prop: Option<impl FuzzerProp<'b, IseBackend<'b>> + 'b>) -> Self {
        if let Some(prop) = prop {
            self.prop(prop)
        } else {
            self
        }
    }

    fn extra_tile<R: TileRelation + 'b>(self, relation: R, bel: impl Into<String>) -> Self {
        self.prop(ExtraTile::new(relation, Some(bel.into()), None, None))
    }

    fn extra_tile_fixed(self, nloc: TileCoord, bel: impl Into<String>) -> Self {
        self.extra_tile(FixedRelation(nloc), bel)
    }

    fn extra_tile_attr<R: TileRelation + 'b>(
        self,
        relation: R,
        bel: impl Into<String>,
        attr: impl Into<String>,
        val: impl Into<String>,
    ) -> Self {
        self.prop(ExtraTile::new(
            relation,
            Some(bel.into()),
            Some(attr.into()),
            Some(val.into()),
        ))
    }

    fn extra_tile_attr_fixed(
        self,
        nloc: TileCoord,
        bel: impl Into<String>,
        attr: impl Into<String>,
        val: impl Into<String>,
    ) -> Self {
        self.extra_tile_attr(FixedRelation(nloc), bel, attr, val)
    }

    fn extra_tiles_by_kind(self, kind: impl AsRef<str>, bel: impl Into<String>) -> Self {
        let kind = self.backend().egrid.db.get_tile_class(kind.as_ref());
        self.prop(ExtraTilesByKind::new(kind, Some(bel.into()), None, None))
    }

    fn extra_tiles_attr_by_kind(
        self,
        kind: impl AsRef<str>,
        bel: impl Into<String>,
        attr: impl Into<String>,
        val: impl Into<String>,
    ) -> Self {
        let kind = self.backend().egrid.db.get_tile_class(kind.as_ref());
        self.prop(ExtraTilesByKind::new(
            kind,
            Some(bel.into()),
            Some(attr.into()),
            Some(val.into()),
        ))
    }

    fn extra_tiles_by_bel(self, slot: BelSlotId, bel: impl Into<String>) -> Self {
        self.prop(ExtraTilesByBel::new(slot, Some(bel.into()), None, None))
    }

    fn extra_tiles_attr_by_bel(
        self,
        slot: BelSlotId,
        bel: impl Into<String>,
        attr: impl Into<String>,
        val: impl Into<String>,
    ) -> Self {
        self.prop(ExtraTilesByBel::new(
            slot,
            Some(bel.into()),
            Some(attr.into()),
            Some(val.into()),
        ))
    }

    fn extra_tile_reg(self, reg: Reg, tile: impl Into<String>, bel: impl Into<String>) -> Self {
        self.prop(ExtraReg::new(
            vec![reg],
            false,
            tile.into(),
            Some(bel.into()),
            None,
            None,
        ))
    }

    fn extra_tile_reg_present(
        self,
        reg: Reg,
        tile: impl Into<String>,
        bel: impl Into<String>,
    ) -> Self {
        self.prop(ExtraReg::new(
            vec![reg],
            true,
            tile.into(),
            Some(bel.into()),
            None,
            None,
        ))
    }

    fn extra_tile_reg_attr(
        self,
        reg: Reg,
        tile: impl Into<String>,
        bel: impl Into<String>,
        attr: impl Into<String>,
        val: impl Into<String>,
    ) -> Self {
        self.prop(ExtraReg::new(
            vec![reg],
            false,
            tile.into(),
            Some(bel.into()),
            Some(attr.into()),
            Some(val.into()),
        ))
    }

    fn null_bits(self) -> Self {
        self.prop(NullBits)
    }

    fn no_related<R: TileRelation + 'b>(self, relation: R) -> Self {
        self.prop(HasRelated::new(relation, false))
    }

    fn has_related<R: TileRelation + 'b>(self, relation: R) -> Self {
        self.prop(HasRelated::new(relation, true))
    }
}

pub struct FuzzBuilder<'sm, 'b> {
    pub session: &'sm mut Session<'b, IseBackend<'b>>,
    pub backend: &'b IseBackend<'b>,
    pub node_kind: Option<TileClassId>,
    pub props: Vec<Box<DynProp<'b>>>,
}

impl<'b> FuzzBuilderBase<'b> for FuzzBuilder<'_, 'b> {
    fn prop_box(mut self, prop: Box<DynProp<'b>>) -> Self {
        self.props.push(prop);
        self
    }

    fn backend(&self) -> &'b IseBackend<'b> {
        self.backend
    }
}

impl<'sm, 'b> FuzzBuilder<'sm, 'b> {
    pub fn test_manual(
        self,
        bel: &'static str,
        attr: impl AsRef<str>,
        val: impl AsRef<str>,
    ) -> FuzzBuilderTestManual<'sm, 'b> {
        let attr = attr.as_ref();
        let val = val.as_ref();
        let feature = FeatureId {
            tile: if let Some(node_kind) = self.node_kind {
                self.backend.egrid.db.tile_classes.key(node_kind).clone()
            } else {
                "NULL".into()
            },
            bel: bel.into(),
            attr: attr.into(),
            val: val.into(),
        };
        FuzzBuilderTestManual {
            session: self.session,
            node_kind: self.node_kind,
            props: self.props,
            feature,
        }
    }

    pub fn test_reg(
        self,
        reg: Reg,
        tile: impl Into<String>,
        bel: &'static str,
        attr: impl AsRef<str>,
        val: impl AsRef<str>,
    ) -> FuzzBuilderTestManual<'sm, 'b> {
        let attr = attr.as_ref();
        let val = val.as_ref();
        self.extra_tile_reg(reg, tile, bel)
            .test_manual(bel, attr, val)
    }

    pub fn test_reg_present(
        self,
        reg: Reg,
        tile: impl Into<String>,
        bel: &'static str,
        attr: impl AsRef<str>,
        val: impl AsRef<str>,
    ) -> FuzzBuilderTestManual<'sm, 'b> {
        let attr = attr.as_ref();
        let val = val.as_ref();
        self.extra_tile_reg_present(reg, tile, bel)
            .test_manual(bel, attr, val)
    }

    pub fn test_gtz(
        self,
        dir: DirV,
        attr: impl AsRef<str>,
        val: impl AsRef<str>,
    ) -> FuzzBuilderTestManual<'sm, 'b> {
        self.prop(ExtraGtz(dir)).test_manual("GTZ", attr, val)
    }
}

#[must_use]
pub struct FuzzBuilderTestManual<'sm, 'b> {
    pub session: &'sm mut Session<'b, IseBackend<'b>>,
    pub node_kind: Option<TileClassId>,
    pub props: Vec<Box<DynProp<'b>>>,
    pub feature: FeatureId,
}

impl<'b> FuzzBuilderTestManual<'_, 'b> {
    pub fn prop(mut self, prop: impl FuzzerProp<'b, IseBackend<'b>> + 'b) -> Self {
        self.props.push(Box::new(prop));
        self
    }

    pub fn prop_box(mut self, prop: Box<DynProp<'b>>) -> Self {
        self.props.push(prop);
        self
    }

    pub fn raw_diff(
        self,
        key: Key<'b>,
        val0: impl Into<Value<'b>>,
        val1: impl Into<Value<'b>>,
    ) -> Self {
        self.prop(FuzzRaw::new(key, val0.into(), val1.into()))
    }

    pub fn raw_multi(self, key: Key<'b>, val: MultiValue, width: usize) {
        self.prop(FuzzRawMulti::new(key, val, width)).commit();
    }

    pub fn global(self, opt: impl Into<String>, val: impl Into<String>) -> Self {
        self.raw_diff(Key::GlobalOpt(opt.into()), None, val.into())
    }

    pub fn global_diff(
        self,
        opt: impl Into<String>,
        val0: impl Into<String>,
        val1: impl Into<String>,
    ) -> Self {
        self.raw_diff(Key::GlobalOpt(opt.into()), val0.into(), val1.into())
    }

    pub fn commit(self) {
        let fgen = FpgaFuzzerGen {
            node_kind: self.node_kind,
            feature: self.feature,
            props: self.props,
        };
        self.session.add_fuzzer(Box::new(fgen));
    }

    pub fn multi_global(self, opt: impl Into<String>, val: MultiValue, width: usize) {
        self.raw_multi(Key::GlobalOpt(opt.into()), val, width);
    }
}

pub struct FuzzCtxBel<'sm, 'b> {
    pub session: &'sm mut Session<'b, IseBackend<'b>>,
    pub backend: &'b IseBackend<'b>,
    pub node_kind: TileClassId,
    pub bel: BelSlotId,
}

impl<'b> FuzzCtxBel<'_, 'b> {
    pub fn build<'sm>(&'sm mut self) -> FuzzBuilderBel<'sm, 'b> {
        FuzzBuilderBel {
            session: &mut *self.session,
            backend: self.backend,
            node_kind: self.node_kind,
            bel: self.bel,
            props: vec![],
        }
    }

    pub fn mode<'sm>(&'sm mut self, mode: impl Into<String>) -> FuzzBuilderBel<'sm, 'b> {
        self.build().mode(mode)
    }

    pub fn test_manual<'sm>(
        &'sm mut self,
        attr: impl AsRef<str>,
        val: impl AsRef<str>,
    ) -> FuzzBuilderBelTestManual<'sm, 'b> {
        self.build().test_manual(attr, val)
    }
}

pub struct FuzzBuilderBel<'sm, 'b> {
    pub session: &'sm mut Session<'b, IseBackend<'b>>,
    pub backend: &'b IseBackend<'b>,
    pub node_kind: TileClassId,
    pub bel: BelSlotId,
    pub props: Vec<Box<DynProp<'b>>>,
}

impl<'b> FuzzBuilderBase<'b> for FuzzBuilderBel<'_, 'b> {
    fn prop_box(mut self, prop: Box<DynProp<'b>>) -> Self {
        self.props.push(prop);
        self
    }

    fn backend(&self) -> &'b IseBackend<'b> {
        self.backend
    }
}

impl<'sm, 'b> FuzzBuilderBel<'sm, 'b> {
    pub fn props(mut self, props: impl IntoIterator<Item = Box<DynProp<'b>>>) -> Self {
        self.props.extend(props);
        self
    }

    pub fn force_bel_name(self, bel_name: impl Into<String>) -> Self {
        self.prop(ForceBelName(bel_name.into()))
    }

    pub fn global_xy(self, opt: impl Into<String>, val: impl Into<String>) -> Self {
        let prop = BaseGlobalXy::new(self.bel, opt.into(), val.into());
        self.prop(prop)
    }

    pub fn mode(self, mode: impl Into<String>) -> Self {
        let bel = self.bel;
        self.bel_mode(bel, mode)
    }

    pub fn bel_mode(self, bel: BelSlotId, mode: impl Into<String>) -> Self {
        let prop = BaseBelMode::new(bel, mode.into());
        self.prop(IntMutex::new("MAIN".into())).prop(prop)
    }

    pub fn unused(self) -> Self {
        let bel = self.bel;
        self.bel_unused(bel)
    }

    pub fn bel_unused(self, bel: BelSlotId) -> Self {
        let prop = BaseBelMode::new(bel, "".into());
        self.prop(prop)
    }

    pub fn global_mutex_here(self, key: impl Into<String>) -> Self {
        let prop = GlobalMutexHere::new(self.bel, key.into());
        self.prop(prop)
    }

    pub fn row_mutex_here(self, key: impl Into<String>) -> Self {
        let prop = RowMutexHere::new(self.bel, key.into());
        self.prop(prop)
    }

    pub fn pin(self, pin: impl Into<String>) -> Self {
        let bel = self.bel;
        self.bel_pin(bel, pin)
    }

    pub fn bel_pin(self, bel: BelSlotId, pin: impl Into<String>) -> Self {
        self.prop(BaseBelPin::new(bel, pin.into()))
    }

    pub fn no_pin(self, pin: impl Into<String>) -> Self {
        let prop = BaseBelNoPin::new(self.bel, pin.into());
        self.prop(prop)
    }

    pub fn pin_pips(self, pin: impl Into<String>) -> Self {
        let prop = BaseBelPinPips::new(self.bel, pin.into());
        self.prop(prop)
    }

    pub fn pin_from(self, pin: impl Into<String>, from: PinFromKind) -> Self {
        let prop = BaseBelPinFrom::new(self.bel, pin.into(), from);
        self.prop(prop)
    }

    pub fn attr(self, attr: impl Into<String>, val: impl Into<String>) -> Self {
        let bel = self.bel;
        self.bel_attr(bel, attr, val)
    }

    pub fn bel_attr(self, bel: BelSlotId, attr: impl Into<String>, val: impl Into<String>) -> Self {
        let prop = BaseBelAttr::new(bel, attr.into(), val.into());
        self.prop(prop)
    }

    pub fn mutex(self, key: impl Into<String>, val: impl Into<String>) -> Self {
        let bel = self.bel;
        self.bel_mutex(bel, key, val)
    }

    pub fn bel_mutex(self, bel: BelSlotId, key: impl Into<String>, val: impl Into<String>) -> Self {
        let prop = BelMutex::new(bel, key.into(), val.into());
        self.prop(prop)
    }

    pub fn pip(self, wire_to: impl BelIntoPipWire, wire_from: impl BelIntoPipWire) -> Self {
        self.related_pip(NoopRelation, wire_to, wire_from)
    }

    pub fn related_pip<R: TileRelation + 'b>(
        self,
        relation: R,
        wire_to: impl BelIntoPipWire,
        wire_from: impl BelIntoPipWire,
    ) -> Self {
        let wire_to = wire_to.into_pip_wire(self.backend, self.bel);
        let wire_from = wire_from.into_pip_wire(self.backend, self.bel);
        let prop = BasePip::new(relation, wire_to, wire_from);
        self.prop(prop)
    }

    pub fn test_enum(self, attr: impl AsRef<str>, vals: &[impl AsRef<str>]) {
        let attr = attr.as_ref();
        for val in vals {
            let val = val.as_ref();
            let feature = FeatureId {
                tile: self
                    .backend
                    .egrid
                    .db
                    .tile_classes
                    .key(self.node_kind)
                    .clone(),
                bel: self.backend.egrid.db.bel_slots.key(self.bel).clone(),
                attr: attr.into(),
                val: val.into(),
            };
            let mut props = Vec::from_iter(self.props.iter().map(|x| x.dyn_clone()));
            props.push(Box::new(FuzzBelAttr::new(
                self.bel,
                attr.into(),
                "".into(),
                val.into(),
            )));
            let fgen = FpgaFuzzerGen {
                node_kind: Some(self.node_kind),
                feature,
                props,
            };
            self.session.add_fuzzer(Box::new(fgen));
        }
    }

    pub fn test_enum_suffix(
        self,
        attr: impl AsRef<str>,
        suffix: impl AsRef<str>,
        vals: &[impl AsRef<str>],
    ) {
        let attr = attr.as_ref();
        let suffix = suffix.as_ref();
        for val in vals {
            let val = val.as_ref();
            let feature = FeatureId {
                tile: self
                    .backend
                    .egrid
                    .db
                    .tile_classes
                    .key(self.node_kind)
                    .clone(),
                bel: self.backend.egrid.db.bel_slots.key(self.bel).clone(),
                attr: format!("{attr}.{suffix}"),
                val: val.into(),
            };
            let mut props = Vec::from_iter(self.props.iter().map(|x| x.dyn_clone()));
            props.push(Box::new(FuzzBelAttr::new(
                self.bel,
                attr.into(),
                "".into(),
                val.into(),
            )));
            let fgen = FpgaFuzzerGen {
                node_kind: Some(self.node_kind),
                feature,
                props,
            };
            self.session.add_fuzzer(Box::new(fgen));
        }
    }

    pub fn test_inv(self, pin: impl Into<String>) {
        let pin = pin.into();
        let pininv = format!("{pin}INV");
        let pin_b = format!("{pin}_B");
        self.pin(&pin).test_enum(pininv, &[pin, pin_b]);
    }

    pub fn test_inv_suffix(self, pin: impl Into<String>, suffix: impl AsRef<str>) {
        let pin = pin.into();
        let pininv = format!("{pin}INV");
        let pin_b = format!("{pin}_B");
        self.pin(&pin)
            .test_enum_suffix(pininv, suffix, &[pin, pin_b]);
    }

    pub fn test_multi_attr_bin(self, attr: impl Into<String>, width: usize) {
        let attr = attr.into();
        let prop = FuzzBelMultiAttr::new(self.bel, attr.clone(), MultiValue::Bin, width);
        self.test_manual(attr, "").prop(prop).commit();
    }

    pub fn test_multi_attr_dec(self, attr: impl Into<String>, width: usize) {
        let attr = attr.into();
        let prop = FuzzBelMultiAttr::new(self.bel, attr.clone(), MultiValue::Dec(0), width);
        self.test_manual(attr, "").prop(prop).commit();
    }

    pub fn test_multi_attr_dec_delta(self, attr: impl Into<String>, width: usize, delta: i32) {
        let attr = attr.into();
        let prop = FuzzBelMultiAttr::new(self.bel, attr.clone(), MultiValue::Dec(delta), width);
        self.test_manual(attr, "").prop(prop).commit();
    }

    pub fn test_multi_attr_hex(self, attr: impl Into<String>, width: usize) {
        let attr = attr.into();
        let prop = FuzzBelMultiAttr::new(self.bel, attr.clone(), MultiValue::Hex(0), width);
        self.test_manual(attr, "").prop(prop).commit();
    }

    pub fn test_multi_attr_lut(self, attr: impl Into<String>, width: usize) {
        let attr = attr.into();
        let prop = FuzzBelMultiAttr::new(self.bel, attr.clone(), MultiValue::Lut, width);
        self.test_manual(attr, "#LUT").prop(prop).commit();
    }

    pub fn test_multi_attr(self, attr: impl Into<String>, value: MultiValue, width: usize) {
        let attr = attr.into();
        let prop = FuzzBelMultiAttr::new(self.bel, attr.clone(), value, width);
        self.test_manual(attr, "").prop(prop).commit();
    }

    pub fn test_manual(
        self,
        attr: impl AsRef<str>,
        val: impl AsRef<str>,
    ) -> FuzzBuilderBelTestManual<'sm, 'b> {
        let attr = attr.as_ref();
        let val = val.as_ref();
        let feature = FeatureId {
            tile: self
                .backend
                .egrid
                .db
                .tile_classes
                .key(self.node_kind)
                .clone(),
            bel: self.backend.egrid.db.bel_slots.key(self.bel).clone(),
            attr: attr.into(),
            val: val.into(),
        };
        FuzzBuilderBelTestManual {
            session: self.session,
            backend: self.backend,
            node_kind: self.node_kind,
            bel: self.bel,
            props: self.props,
            feature,
        }
    }
}

#[must_use]
pub struct FuzzBuilderBelTestManual<'sm, 'b> {
    pub session: &'sm mut Session<'b, IseBackend<'b>>,
    pub backend: &'b IseBackend<'b>,
    pub node_kind: TileClassId,
    pub bel: BelSlotId,
    pub props: Vec<Box<DynProp<'b>>>,
    pub feature: FeatureId,
}

impl<'b> FuzzBuilderBelTestManual<'_, 'b> {
    pub fn prop(mut self, prop: impl FuzzerProp<'b, IseBackend<'b>> + 'b) -> Self {
        self.props.push(Box::new(prop));
        self
    }

    pub fn raw_diff(
        self,
        key: Key<'b>,
        val0: impl Into<Value<'b>>,
        val1: impl Into<Value<'b>>,
    ) -> Self {
        self.prop(FuzzRaw::new(key, val0.into(), val1.into()))
    }

    pub fn raw_multi(self, key: Key<'b>, val: MultiValue, width: usize) {
        self.prop(FuzzRawMulti::new(key, val, width)).commit();
    }

    pub fn global(self, opt: impl Into<String>, val: impl Into<String>) -> Self {
        self.raw_diff(Key::GlobalOpt(opt.into()), None, val.into())
    }

    pub fn global_diff(
        self,
        opt: impl Into<String>,
        val0: impl Into<String>,
        val1: impl Into<String>,
    ) -> Self {
        self.raw_diff(Key::GlobalOpt(opt.into()), val0.into(), val1.into())
    }

    pub fn global_xy(self, opt: impl Into<String>, val: impl Into<String>) -> Self {
        let prop = FuzzGlobalXy::new(self.bel, opt.into(), None, Some(val.into()));
        self.prop(prop)
    }

    pub fn mode(self, mode: impl Into<String>) -> Self {
        let mode = mode.into();
        let prop = FuzzBelMode::new(self.bel, "".into(), mode);
        self.prop(IntMutex::new("MAIN".into())).prop(prop)
    }

    pub fn mode_diff(self, mode0: impl Into<String>, mode1: impl Into<String>) -> Self {
        let bel = self.bel;
        self.bel_mode_diff(bel, mode0, mode1)
    }

    pub fn bel_mode_diff(
        self,
        bel: BelSlotId,
        mode0: impl Into<String>,
        mode1: impl Into<String>,
    ) -> Self {
        let mode0 = mode0.into();
        let mode1 = mode1.into();
        let prop = FuzzBelMode::new(bel, mode0, mode1);
        self.prop(IntMutex::new("MAIN".into())).prop(prop)
    }

    pub fn attr(self, attr: impl Into<String>, val: impl Into<String>) -> Self {
        let bel = self.bel;
        self.bel_attr(bel, attr, val)
    }

    pub fn bel_attr(self, bel: BelSlotId, attr: impl Into<String>, val: impl Into<String>) -> Self {
        let prop = FuzzBelAttr::new(bel, attr.into(), "".into(), val.into());
        self.prop(prop)
    }

    pub fn attr_diff(
        self,
        attr: impl Into<String>,
        val_a: impl Into<String>,
        val_b: impl Into<String>,
    ) -> Self {
        let bel = self.bel;
        self.bel_attr_diff(bel, attr, val_a, val_b)
    }

    pub fn bel_attr_diff(
        self,
        bel: BelSlotId,
        attr: impl Into<String>,
        val_a: impl Into<String>,
        val_b: impl Into<String>,
    ) -> Self {
        let prop = FuzzBelAttr::new(bel, attr.into(), val_a.into(), val_b.into());
        self.prop(prop)
    }

    pub fn pin(self, pin: impl Into<String>) -> Self {
        let bel = self.bel;
        self.bel_pin(bel, pin)
    }

    pub fn bel_pin(self, bel: BelSlotId, pin: impl Into<String>) -> Self {
        let prop = FuzzBelPin::new(bel, pin.into());
        self.prop(prop)
    }

    pub fn pin_pips(self, pin: impl Into<String>) -> Self {
        let prop = FuzzBelPinPips::new(self.bel, pin.into());
        self.prop(prop)
    }

    pub fn pin_int_pips(self, pin: impl Into<String>) -> Self {
        let prop = FuzzBelPinIntPips::new(self.bel, pin.into());
        self.prop(prop)
    }

    pub fn pin_from(self, pin: impl Into<String>, from0: PinFromKind, from1: PinFromKind) -> Self {
        let prop = FuzzBelPinFrom::new(self.bel, pin.into(), from0, from1);
        self.prop(prop)
    }

    pub fn pip(self, wire_to: impl BelIntoPipWire, wire_from: impl BelIntoPipWire) -> Self {
        self.related_pip(NoopRelation, wire_to, wire_from)
    }

    pub fn pin_pair(
        self,
        pin_to: impl Into<String>,
        bel_from: BelSlotId,
        pin_from: impl Into<String>,
    ) -> Self {
        let prop = FuzzBelPinPair::new(self.bel, pin_to.into(), bel_from, pin_from.into());
        self.prop(prop)
    }

    pub fn related_pip<R: TileRelation + 'b>(
        self,
        relation: R,
        wire_to: impl BelIntoPipWire,
        wire_from: impl BelIntoPipWire,
    ) -> Self {
        let wire_to = wire_to.into_pip_wire(self.backend, self.bel);
        let wire_from = wire_from.into_pip_wire(self.backend, self.bel);
        let prop = FuzzPip::new(relation, wire_to, wire_from);
        self.prop(prop)
    }

    pub fn commit(self) {
        let fgen = FpgaFuzzerGen {
            node_kind: Some(self.node_kind),
            feature: self.feature,
            props: self.props,
        };
        self.session.add_fuzzer(Box::new(fgen));
    }

    pub fn multi_global(self, opt: impl Into<String>, val: MultiValue, width: usize) {
        self.raw_multi(Key::GlobalOpt(opt.into()), val, width);
    }

    pub fn multi_attr(self, attr: impl Into<String>, val: MultiValue, width: usize) {
        let attr = attr.into();
        let prop = FuzzBelMultiAttr::new(self.bel, attr, val, width);
        self.prop(prop).commit();
    }

    pub fn multi_global_xy(self, opt: impl Into<String>, val: MultiValue, width: usize) {
        let opt = opt.into();
        let prop = FuzzMultiGlobalXy::new(self.bel, opt, val, width);
        self.prop(prop).commit();
    }
}
