//! The Itanium vtable-group builder: a port of clang 19's
//! `VCallAndVBaseOffsetBuilder` and `ItaniumVTableBuilder`
//! (`VTableBuilder.cpp`), function by function, so that the components,
//! address points, thunks and indices match clang's exactly.

use std::collections::BTreeMap;
use std::rc::Rc;

use kiln_ast::DeclId;
use kiln_ast::class::{Thunk, VTableComponent};
use kiln_ast::types::TypeView;
use kiln_source::fx::{FxHashMap, FxHashSet};

use super::VTableError;
use super::classes::{
    Classes, can_share_vcall_offset, implicit_rank, is_dtor, is_implicit, is_pure, parent,
    record_of, return_type,
};
use super::overriders::{FinalOverriders, OverriderInfo};
use super::template::{Diff, Off, Syms};

/// clang `VCallOffsetMap`: methods and their vcall-offset offsets.
#[derive(Clone, Debug, Default)]
pub(crate) struct VCallOffsetMap {
    offsets: Vec<(DeclId, i64)>,
}

impl VCallOffsetMap {
    fn add(&mut self, cx: &Classes<'_>, m: DeclId, oo: i64) -> bool {
        let ast = cx.ast();
        if self.offsets.iter().any(|&(o, _)| can_share_vcall_offset(ast, o, m)) {
            return false;
        }
        self.offsets.push((m, oo));
        true
    }

    fn get(&self, cx: &Classes<'_>, m: DeclId) -> Option<i64> {
        let ast = cx.ast();
        self.offsets.iter().find(|&&(o, _)| can_share_vcall_offset(ast, o, m)).map(|&(_, oo)| oo)
    }

    fn is_empty(&self) -> bool {
        self.offsets.is_empty()
    }
}

/// The vcall and vbase offsets above one vtable's offset-to-top.
pub(crate) struct OffsetsResult {
    /// In vtable order (clang's `components_begin..end`, reversed).
    pub components: Vec<VTableComponent>,
    /// Per component, its value as a form (recording builds only).
    pub diffs: Vec<Option<Diff>>,
    pub vcall: VCallOffsetMap,
    pub vbase_offset_offsets: Vec<(DeclId, i64)>,
}

/// clang `VCallAndVBaseOffsetBuilder`.
struct OffsetsBuilder<'o> {
    md: DeclId,
    syms: &'o Syms,
    ov: Option<&'o FinalOverriders>,
    components: Vec<VTableComponent>,
    diffs: Vec<Option<Diff>>,
    vcall: VCallOffsetMap,
    vbase_oo: Vec<(DeclId, i64)>,
    visited_vbases: FxHashSet<DeclId>,
    /// Classes whose virtual bases are all visited already.
    explored: FxHashSet<DeclId>,
    ptr: i64,
}

/// Runs clang's `VCallAndVBaseOffsetBuilder` constructor (offsets in
/// the layout class of `syms`).
pub(crate) fn build_offsets(
    cx: &mut Classes<'_>,
    md: DeclId,
    syms: &Syms,
    ov: Option<&FinalOverriders>,
    base: (DeclId, i64),
    base_is_virtual: bool,
    off_in_layout: Off,
) -> Result<OffsetsResult, VTableError> {
    let mut b = OffsetsBuilder {
        md,
        syms,
        ov,
        components: Vec::new(),
        diffs: Vec::new(),
        vcall: VCallOffsetMap::default(),
        vbase_oo: Vec::new(),
        visited_vbases: FxHashSet::default(),
        explored: FxHashSet::default(),
        ptr: cx.ptr_bytes(),
    };
    b.add_vcall_and_vbase_offsets(cx, base, base_is_virtual, off_in_layout)?;
    b.components.reverse();
    b.diffs.reverse();
    Ok(OffsetsResult {
        components: b.components,
        diffs: b.diffs,
        vcall: b.vcall,
        vbase_offset_offsets: b.vbase_oo,
    })
}

/// [`build_offsets`] with `rd` as its own layout class, for the vcall and
/// vbase-offset offsets alone (no recording).
fn own_offsets(
    cx: &mut Classes<'_>,
    rd: DeclId,
    base: DeclId,
    base_is_virtual: bool,
) -> Result<OffsetsResult, VTableError> {
    build_offsets(cx, rd, &Syms::concrete(rd), None, (base, 0), base_is_virtual, Off::abs(0))
}

impl OffsetsBuilder<'_> {
    fn current_offset_offset(&self) -> i64 {
        let n = i64::try_from(self.components.len()).unwrap_or(i64::MAX / 16);
        -(3 + n) * self.ptr
    }

    fn add_vcall_and_vbase_offsets(
        &mut self,
        cx: &mut Classes<'_>,
        base: (DeclId, i64),
        base_is_virtual: bool,
        real_offset: Off,
    ) -> Result<(), VTableError> {
        cx.tick()?;
        let info = cx.get(base.0);
        if let Some((p, pv)) = info.primary {
            let po = if pv { cx.get(self.md).vbase_offset(p) } else { base.1 };
            self.add_vcall_and_vbase_offsets(cx, (p, po), pv, real_offset)?;
        }
        self.add_vbase_offsets(cx, base.0, real_offset)?;
        if base_is_virtual {
            self.add_vcall_offsets(cx, base, real_offset)?;
        }
        Ok(())
    }

    fn add_vcall_offsets(
        &mut self,
        cx: &mut Classes<'_>,
        base: (DeclId, i64),
        vbase_offset: Off,
    ) -> Result<(), VTableError> {
        cx.tick()?;
        let info = cx.get(base.0);
        if let Some((p, false)) = info.primary {
            self.add_vcall_offsets(cx, (p, base.1), vbase_offset)?;
        }
        for &m in &info.slots {
            let oo = self.current_offset_offset();
            if !self.vcall.add(cx, m, oo) {
                continue;
            }
            let (offset, diff) = match self.ov {
                Some(ov) => {
                    let o = ov.get(m, base.1)?.offset;
                    (o.val_minus(vbase_offset), Some(o.minus(vbase_offset)))
                }
                None => (0, None),
            };
            self.push(VTableComponent::VCallOffset(offset), diff);
        }
        let primary = info.primary.map(|p| p.0);
        for &(b, v) in &info.bases {
            if v || Some(b) == primary || !cx.get(b).polymorphic {
                continue;
            }
            self.add_vcall_offsets(cx, (b, base.1 + info.base_offset(b)), vbase_offset)?;
        }
        Ok(())
    }

    fn add_vbase_offsets(
        &mut self,
        cx: &mut Classes<'_>,
        rd: DeclId,
        off_in_layout: Off,
    ) -> Result<(), VTableError> {
        cx.tick()?;
        let info = cx.get(rd);
        if !info.has_vbases() || self.explored.contains(&rd) {
            return Ok(());
        }
        for &(b, v) in &info.bases {
            if v && self.visited_vbases.insert(b) {
                let l = self.syms.vbase(cx, b);
                let oo = self.current_offset_offset();
                self.vbase_oo.push((b, oo));
                let diff = Some(l.minus(off_in_layout));
                self.push(VTableComponent::VBaseOffset(l.val_minus(off_in_layout)), diff);
            }
            self.add_vbase_offsets(cx, b, off_in_layout)?;
        }
        self.explored.insert(rd);
        Ok(())
    }

    fn push(&mut self, c: VTableComponent, diff: Option<Diff>) {
        self.components.push(c);
        if self.syms.is_symbolic() {
            self.diffs.push(diff);
        }
    }
}

/// clang `BaseOffset`: from a derived class to one of its bases.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BaseOffset {
    derived: Option<DeclId>,
    vbase: Option<DeclId>,
    nv: i64,
}

impl BaseOffset {
    const EMPTY: BaseOffset = BaseOffset { derived: None, vbase: None, nv: 0 };

    fn is_empty(&self) -> bool {
        self.vbase.is_none() && self.nv == 0
    }
}

/// clang `ItaniumVTableBuilder::MethodInfo`.
#[derive(Clone, Copy, Debug)]
struct MethodInfo {
    base_offset: i64,
    layout_off: Off,
    index: u32,
}

/// A built vtable group, in clang's own terms (base subobjects as
/// `(class, offset in the layout class)`).
#[derive(Clone, Debug)]
pub(crate) struct Built {
    pub most_derived: DeclId,
    pub md_offset: i64,
    pub layout_class: DeclId,
    pub components: Vec<VTableComponent>,
    pub vtable_starts: Vec<u32>,
    /// Address points in insertion order (the first per subobject).
    pub address_points: Vec<((DeclId, i64), u32)>,
    pub slot_thunks: BTreeMap<u32, Thunk>,
    /// Per overrider, its distinct thunks in insertion order.
    pub thunks: Vec<(DeclId, Vec<Thunk>)>,
    pub vbase_offset_offsets: Vec<(DeclId, i64)>,
    /// Dtors give their complete-object index.
    pub method_indices: Vec<(DeclId, u32)>,
}

impl Built {
    /// Is this a construction vtable group?
    pub fn is_construction(&self) -> bool {
        self.most_derived != self.layout_class
    }
}

/// clang `ItaniumVTableBuilder`.
struct VTableBuilder<'c, 'a> {
    cx: &'c mut Classes<'a>,
    md: DeclId,
    md_offset: i64,
    layout_class: DeclId,
    syms: Syms,
    ov: FinalOverriders,
    components: Vec<VTableComponent>,
    vtable_starts: Vec<u32>,
    address_points: Vec<((DeclId, Off), u32)>,
    ap_seen: FxHashMap<(DeclId, i64), Off>,
    vbase_offset_offsets: Vec<(DeclId, i64)>,
    vcall_offsets_for_vbases: FxHashMap<DeclId, VCallOffsetMap>,
    method_info: FxHashMap<DeclId, MethodInfo>,
    slot_thunks: BTreeMap<u32, Thunk>,
    thunks: Vec<(DeclId, Vec<Thunk>)>,
    method_indices: Vec<(DeclId, u32)>,
    primary_virtual_bases: FxHashSet<DeclId>,
    original: FxHashMap<DeclId, DeclId>,
}

/// Builds the vtable group of `most_derived` laid out at `md_offset`
/// (virtually iff `md_virtual`) in `layout_class` (clang
/// `ItaniumVTableBuilder` + `LayoutVTable`).
pub(crate) fn build(
    cx: &mut Classes<'_>,
    most_derived: DeclId,
    md_offset: i64,
    md_virtual: bool,
    layout_class: DeclId,
) -> Result<Built, VTableError> {
    let construction = most_derived != layout_class;
    let syms = if construction {
        let syms = Syms::symbolic(cx, most_derived, md_offset, layout_class);
        if let Some(t) = cx.templates.get(&most_derived)
            && let Some(b) = t.instantiate(&syms, md_virtual, md_offset)
        {
            cx.template_counts.0 = cx.template_counts.0.saturating_add(1);
            return Ok(b);
        }
        cx.template_counts.1 = cx.template_counts.1.saturating_add(1);
        syms
    } else {
        Syms::concrete(layout_class)
    };
    let md_off = syms.md(md_offset);
    let ov = FinalOverriders::new(cx, most_derived, md_off, &syms)?;
    let mut b = VTableBuilder {
        cx,
        md: most_derived,
        md_offset,
        layout_class,
        syms,
        ov,
        components: Vec::new(),
        vtable_starts: Vec::new(),
        address_points: Vec::new(),
        ap_seen: FxHashMap::default(),
        vbase_offset_offsets: Vec::new(),
        vcall_offsets_for_vbases: FxHashMap::default(),
        method_info: FxHashMap::default(),
        slot_thunks: BTreeMap::new(),
        thunks: Vec::new(),
        method_indices: Vec::new(),
        primary_virtual_bases: FxHashSet::default(),
        original: FxHashMap::default(),
    };
    b.layout_primary_and_secondary((most_derived, 0), false, md_virtual, md_off)?;
    let mut vbases = FxHashSet::default();
    let mut memo = FxHashMap::default();
    b.determine_primary_virtual_bases(most_derived, md_off, &mut vbases, &mut memo)?;
    vbases.clear();
    let mut explored = FxHashSet::default();
    b.layout_vtables_for_virtual_bases(most_derived, &mut vbases, &mut explored)?;
    b.method_indices.sort_unstable_by_key(|&(m, i)| (i, m));
    let built = Built {
        most_derived,
        md_offset,
        layout_class,
        components: b.components,
        vtable_starts: b.vtable_starts,
        address_points: b.address_points.iter().map(|&((c, o), ap)| ((c, o.val), ap)).collect(),
        slot_thunks: b.slot_thunks,
        thunks: b.thunks,
        vbase_offset_offsets: b.vbase_offset_offsets,
        method_indices: b.method_indices,
    };
    if let Some(t) = b.syms.into_template(&built, md_virtual, b.address_points) {
        b.cx.templates.insert(most_derived, t);
    }
    Ok(built)
}

fn index(n: usize) -> Result<u32, VTableError> {
    u32::try_from(n).map_err(|_| VTableError::TooComplex)
}

impl VTableBuilder<'_, '_> {
    fn construction(&self) -> bool {
        self.md != self.layout_class
    }

    fn md_vbase_offset(&mut self, v: DeclId) -> i64 {
        self.cx.get(self.md).vbase_offset(v)
    }

    fn layout_vbase_offset(&mut self, v: DeclId) -> Off {
        self.syms.vbase(self.cx, v)
    }

    fn add_thunk(&mut self, m: DeclId, t: Thunk) {
        if self.construction() {
            return;
        }
        match self.thunks.iter_mut().find(|(x, _)| *x == m) {
            Some((_, list)) => {
                if !list.contains(&t) {
                    list.push(t);
                }
            }
            None => self.thunks.push((m, vec![t])),
        }
    }

    /// clang `ComputeReturnAdjustmentBaseOffset`.
    fn return_adjustment_base_offset(
        &mut self,
        derived_md: DeclId,
        base_md: DeclId,
    ) -> Result<BaseOffset, VTableError> {
        let ast = self.cx.ast();
        let (Some(dr), Some(br)) = (return_type(ast, derived_md), return_type(ast, base_md)) else {
            return Err(VTableError::Incomplete);
        };
        if dr == br {
            return Ok(BaseOffset::EMPTY);
        }
        let pointee = |q| match ast.types.canonical_view(q) {
            TypeView::Pointer(p) | TypeView::LValueRef(p) | TypeView::RValueRef(p) => Some(p),
            _ => None,
        };
        let (Some(dp), Some(bp)) = (pointee(dr), pointee(br)) else {
            return Err(VTableError::Incomplete);
        };
        let (dp, bp) = (ast.types.canonical(dp), ast.types.canonical(bp));
        if dp.without_quals() == bp.without_quals() {
            return Ok(BaseOffset::EMPTY);
        }
        let (Some(drd), Some(brd)) = (record_of(ast, dp), record_of(ast, bp)) else {
            return Err(VTableError::Incomplete);
        };
        let paths = self.cx.paths_to(drd, brd)?;
        let e = paths.first().ok_or(VTableError::Incomplete)?;
        Ok(BaseOffset { derived: Some(drd), vbase: e.vbase, nv: e.nv })
    }

    /// clang `ComputeThisAdjustmentBaseOffset`.
    fn this_adjustment_base_offset(
        &mut self,
        base: (DeclId, Off),
        derived: (DeclId, Off),
    ) -> Result<BaseOffset, VTableError> {
        if base.0 == derived.0 {
            return Err(VTableError::Incomplete);
        }
        let paths = self.cx.paths_to(derived.0, base.0)?;
        let mut found = false;
        for e in &paths {
            found = true;
            let off = match e.vbase {
                Some(v) => self.layout_vbase_offset(v),
                None => derived.1,
            }
            .add(e.nv);
            if self.syms.eq(off, base.1) {
                return Ok(BaseOffset { derived: Some(derived.0), vbase: e.vbase, nv: -e.nv });
            }
        }
        if found { Ok(BaseOffset::EMPTY) } else { Err(VTableError::Incomplete) }
    }

    /// clang `ComputeThisAdjustment`.
    fn this_adjustment(
        &mut self,
        md: DeclId,
        layout_off: Off,
        overrider: OverriderInfo,
    ) -> Result<(i64, i64), VTableError> {
        let ast = self.cx.ast();
        if is_pure(ast, overrider.method) {
            return Ok((0, 0));
        }
        let off = self.this_adjustment_base_offset(
            (parent(ast, md), layout_off),
            (parent(ast, overrider.method), overrider.offset),
        )?;
        if off.is_empty() {
            return Ok((0, 0));
        }
        let mut vcall = 0;
        if let Some(v) = off.vbase {
            let empty = self.vcall_offsets_for_vbases.get(&v).is_none_or(VCallOffsetMap::is_empty);
            if empty {
                let r = own_offsets(self.cx, self.md, v, true)?;
                self.vcall_offsets_for_vbases.insert(v, r.vcall);
            }
            let map = self.vcall_offsets_for_vbases.get(&v).ok_or(VTableError::Incomplete)?;
            vcall = map.get(self.cx, md).ok_or(VTableError::Incomplete)?;
        }
        Ok((off.nv, vcall))
    }

    /// clang `ComputeReturnAdjustment`.
    fn return_adjustment(&mut self, off: BaseOffset) -> Result<(i64, i64), VTableError> {
        if off.is_empty() {
            return Ok((0, 0));
        }
        let mut vboo = 0;
        if let (Some(v), Some(derived)) = (off.vbase, off.derived) {
            vboo = if derived == self.md {
                self.vbase_offset_offsets.iter().find(|(d, _)| *d == v).map_or(0, |&(_, o)| o)
            } else {
                vbase_offset_offset(self.cx, derived, v)?
            };
        }
        Ok((off.nv, vboo))
    }

    /// clang `ItaniumVTableContext::findOriginalMethodInMap`.
    fn find_original(&self, mut m: DeclId) -> DeclId {
        let mut guard = 0;
        while let Some(&o) = self.original.get(&m) {
            m = o;
            guard += 1;
            if guard > self.original.len() {
                break;
            }
        }
        m
    }

    fn add_method(&mut self, m: DeclId, ret: (i64, i64)) -> Result<(), VTableError> {
        let ast = self.cx.ast();
        if is_dtor(ast, m) {
            self.components.push(VTableComponent::CompleteDtor(m));
            self.components.push(VTableComponent::DeletingDtor(m));
        } else {
            if ret != (0, 0) {
                self.slot_thunks.entry(index(self.components.len())?).or_default().ret = ret;
            }
            self.components.push(VTableComponent::Func(m));
        }
        Ok(())
    }

    /// clang `IsOverriderUsed`.
    fn is_overrider_used(
        &mut self,
        overrider: DeclId,
        layout_off: Off,
        first_base: DeclId,
        first_off: Off,
    ) -> Result<bool, VTableError> {
        let ast = self.cx.ast();
        if self.syms.eq(layout_off, first_off) || parent(ast, overrider) == first_base {
            return Ok(true);
        }
        let mut chain: FxHashSet<DeclId> = FxHashSet::default();
        chain.insert(first_base);
        let mut rd = first_base;
        loop {
            self.cx.tick()?;
            let info = self.cx.get(rd);
            let Some((p, pv)) = info.primary else { break };
            if pv {
                let l = self.layout_vbase_offset(p);
                if !self.syms.eq(l, first_off) {
                    break;
                }
            }
            if !chain.insert(p) {
                return Err(VTableError::Incomplete);
            }
            rd = p;
        }
        if chain.contains(&parent(ast, overrider)) {
            return Ok(true);
        }
        Ok(self.cx.all_overridden(overrider).iter().any(|&o| chain.contains(&parent(ast, o))))
    }

    /// clang `AddMethods`.
    fn add_methods(
        &mut self,
        base: (DeclId, i64),
        layout_off: Off,
        first_base: DeclId,
        first_off: Off,
        primary_bases: &mut PrimaryBases,
    ) -> Result<(), VTableError> {
        self.cx.tick()?;
        let ast = self.cx.ast();
        let info = self.cx.get(base.0);
        if let Some((p, pv)) = info.primary {
            let (po, plo) = if pv {
                (self.md_vbase_offset(p), self.layout_vbase_offset(p))
            } else {
                (base.1, layout_off)
            };
            self.add_methods((p, po), plo, first_base, first_off, primary_bases)?;
            if !primary_bases.push(p) {
                return Err(VTableError::Incomplete);
            }
        }
        let mut new_fns = Vec::new();
        let mut new_implicit = Vec::new();
        for &m in &info.slots {
            let overrider = self.ov.get(m, base.1)?;
            if let Some(om) = self.find_nearest_overridden(m, primary_bases)
                && self.return_adjustment_base_offset(m, om)?.is_empty()
            {
                self.original.insert(m, om);
                let idx = self.method_info.get(&om).map_or(0, |i| i.index);
                self.method_info.entry(m).or_insert(MethodInfo {
                    base_offset: base.1,
                    layout_off,
                    index: idx,
                });
                self.method_info.remove(&om);
                if !self.construction() && om != m {
                    let this = self.this_adjustment(om, layout_off, overrider)?;
                    if this.1 != 0 && parent(ast, overrider.method) == self.md {
                        let rbo = self.return_adjustment_base_offset(overrider.method, m)?;
                        let ret = self.return_adjustment(rbo)?;
                        let t =
                            Thunk { this, ret, method: None, this_class: Some(parent(ast, om)) };
                        self.add_thunk(overrider.method, t);
                    }
                }
                continue;
            }
            if is_implicit(ast, m) {
                new_implicit.push(m);
            } else {
                new_fns.push(m);
            }
        }
        new_implicit.sort_by_key(|&m| implicit_rank(ast, m));
        new_fns.extend(new_implicit);
        for m in new_fns {
            let overrider = self.ov.get(m, base.1)?;
            let idx = index(self.components.len())?;
            self.method_info.entry(m).or_insert(MethodInfo {
                base_offset: base.1,
                layout_off,
                index: idx,
            });
            if !self.is_overrider_used(overrider.method, layout_off, first_base, first_off)? {
                self.components.push(VTableComponent::UnusedFunc(overrider.method));
                continue;
            }
            let rbo = if is_pure(ast, overrider.method) {
                BaseOffset::EMPTY
            } else {
                self.return_adjustment_base_offset(overrider.method, m)?
            };
            let ret = self.return_adjustment(rbo)?;
            if ret != (0, 0) {
                let t = self.slot_thunks.entry(idx).or_default();
                t.method = Some(m);
                t.this_class = Some(parent(ast, m));
            }
            self.add_method(overrider.method, ret)?;
        }
        Ok(())
    }

    /// clang `ComputeThisAdjustments`.
    fn compute_this_adjustments(&mut self) -> Result<(), VTableError> {
        let ast = self.cx.ast();
        let mut infos: Vec<(DeclId, MethodInfo)> = self.method_info.drain().collect();
        infos.sort_unstable_by_key(|&(m, i)| (i.index, m));
        for (m, mi) in infos {
            let slot = mi.index as usize;
            if matches!(self.components.get(slot), Some(VTableComponent::UnusedFunc(_)) | None) {
                continue;
            }
            let overrider = self.ov.get(m, mi.base_offset)?;
            if self.syms.eq(mi.layout_off, overrider.offset)
                && self.slot_thunks.get(&mi.index).is_none_or(|t| t.ret == (0, 0))
            {
                continue;
            }
            let this = self.this_adjustment(m, mi.layout_off, overrider)?;
            if this == (0, 0) {
                continue;
            }
            let original = self.find_original(m);
            let mut set = |i: u32| {
                let t = self.slot_thunks.entry(i).or_insert_with(|| Thunk {
                    method: Some(original),
                    this_class: Some(parent(ast, original)),
                    ..Thunk::default()
                });
                t.this = this;
            };
            set(mi.index);
            if is_dtor(ast, m) {
                set(mi.index + 1);
            }
        }
        if self.construction() {
            return Ok(());
        }
        let slots: Vec<(u32, Thunk)> = self.slot_thunks.iter().map(|(&i, &t)| (i, t)).collect();
        for (i, t) in slots {
            let m = match self.components.get(i as usize) {
                Some(VTableComponent::Func(m) | VTableComponent::CompleteDtor(m)) => *m,
                _ => continue,
            };
            if parent(ast, m) == self.md {
                self.add_thunk(m, t);
            }
        }
        Ok(())
    }

    /// clang `LayoutPrimaryAndSecondaryVTables`.
    fn layout_primary_and_secondary(
        &mut self,
        base: (DeclId, i64),
        morally_virtual: bool,
        virtual_in_layout: bool,
        layout_off: Off,
    ) -> Result<(), VTableError> {
        self.cx.tick()?;
        let ast = self.cx.ast();
        let start = self.components.len();
        self.vtable_starts.push(index(start)?);
        let r = build_offsets(
            self.cx,
            self.md,
            &self.syms,
            Some(&self.ov),
            base,
            virtual_in_layout,
            layout_off,
        )?;
        for (k, d) in r.diffs.iter().enumerate() {
            if let Some(d) = d {
                self.syms.record(start + k, *d);
            }
        }
        self.components.extend(r.components);
        if virtual_in_layout && !r.vcall.is_empty() {
            let e = self.vcall_offsets_for_vbases.entry(base.0).or_default();
            if e.is_empty() {
                *e = r.vcall;
            }
        }
        if base.0 == self.md {
            self.vbase_offset_offsets = r.vbase_offset_offsets;
        }
        let md_off = self.syms.md(self.md_offset);
        self.syms.record(self.components.len(), md_off.minus(layout_off));
        self.components.push(VTableComponent::OffsetToTop(md_off.val_minus(layout_off)));
        self.components.push(VTableComponent::Rtti(self.md));
        let ap = index(self.components.len())?;
        let mut primary_bases = PrimaryBases::default();
        self.add_methods(base, layout_off, base.0, layout_off, &mut primary_bases)?;
        if base.0 == self.md {
            for (&m, mi) in &self.method_info {
                self.method_indices.push((m, mi.index.saturating_sub(ap)));
            }
            let _ = ast;
        }
        self.compute_this_adjustments()?;
        let mut rd = base.0;
        loop {
            if self.syms.insert(&mut self.ap_seen, rd, layout_off) {
                self.address_points.push(((rd, layout_off), ap));
            }
            let info = self.cx.get(rd);
            let Some((p, pv)) = info.primary else { break };
            if pv {
                let l = self.layout_vbase_offset(p);
                if !self.syms.eq(l, layout_off) {
                    break;
                }
            }
            rd = p;
            self.cx.tick()?;
        }
        self.layout_secondary(base, morally_virtual, layout_off)
    }

    /// clang `LayoutSecondaryVTables`.
    fn layout_secondary(
        &mut self,
        base: (DeclId, i64),
        morally_virtual: bool,
        layout_off: Off,
    ) -> Result<(), VTableError> {
        self.cx.tick()?;
        let info = self.cx.get(base.0);
        let primary = info.primary.map(|p| p.0);
        for &(b, v) in &info.bases {
            if v {
                continue;
            }
            let binfo = self.cx.get(b);
            if !binfo.dynamic {
                continue;
            }
            if self.construction() && !morally_virtual && !binfo.has_vbases() {
                continue;
            }
            let rel = info.base_offset(b);
            let (bo, blo) = (base.1 + rel, layout_off.add(rel));
            if Some(b) == primary {
                self.layout_secondary((b, bo), morally_virtual, blo)?;
            } else {
                self.layout_primary_and_secondary((b, bo), morally_virtual, false, blo)?;
            }
        }
        Ok(())
    }

    /// clang `DeterminePrimaryVirtualBases`.
    fn determine_primary_virtual_bases(
        &mut self,
        rd: DeclId,
        layout_off: Off,
        vbases: &mut FxHashSet<DeclId>,
        memo: &mut FxHashMap<(DeclId, i64), Off>,
    ) -> Result<(), VTableError> {
        self.cx.tick()?;
        let info = self.cx.get(rd);
        if !info.has_vbases() || !self.syms.insert(memo, rd, layout_off) {
            return Ok(());
        }
        if let Some((p, true)) = info.primary {
            let primary = !self.construction() || {
                let l = self.layout_vbase_offset(p);
                self.syms.eq(l, layout_off)
            };
            if primary {
                self.primary_virtual_bases.insert(p);
            }
        }
        for &(b, v) in &info.bases {
            let bo = if v {
                if !vbases.insert(b) {
                    continue;
                }
                self.layout_vbase_offset(b)
            } else {
                layout_off.add(info.base_offset(b))
            };
            self.determine_primary_virtual_bases(b, bo, vbases, memo)?;
        }
        Ok(())
    }

    /// clang `LayoutVTablesForVirtualBases`.
    fn layout_vtables_for_virtual_bases(
        &mut self,
        rd: DeclId,
        vbases: &mut FxHashSet<DeclId>,
        explored: &mut FxHashSet<DeclId>,
    ) -> Result<(), VTableError> {
        self.cx.tick()?;
        let info = self.cx.get(rd);
        for &(b, v) in &info.bases {
            let binfo = self.cx.get(b);
            if v && binfo.dynamic && !self.primary_virtual_bases.contains(&b) && vbases.insert(b) {
                let bo = self.md_vbase_offset(b);
                let blo = self.layout_vbase_offset(b);
                self.layout_primary_and_secondary((b, bo), true, true, blo)?;
            }
            if binfo.has_vbases() && explored.insert(b) {
                self.layout_vtables_for_virtual_bases(b, vbases, explored)?;
            }
        }
        Ok(())
    }
}

/// The primary bases `add_methods` has visited, in order, with a set
/// for the membership tests (a chain of depth `n` asks `n` times).
#[derive(Default)]
struct PrimaryBases {
    list: Vec<DeclId>,
    set: FxHashSet<DeclId>,
}

impl PrimaryBases {
    /// Appends `b`; false if it is already there.
    fn push(&mut self, b: DeclId) -> bool {
        if !self.set.insert(b) {
            return false;
        }
        self.list.push(b);
        true
    }
}

impl VTableBuilder<'_, '_> {
    /// clang `FindNearestOverriddenMethod`: the method `m` overrides
    /// whose class is the nearest of `bases` (searched from the last).
    /// The common case, `m`'s first direct override declared in the
    /// last primary base, is the answer without the transitive set.
    fn find_nearest_overridden(&mut self, m: DeclId, bases: &PrimaryBases) -> Option<DeclId> {
        let ast = self.cx.ast();
        let last = *bases.list.last()?;
        let direct = super::classes::overridden(ast, m);
        if let Some(&o) = direct.first()
            && parent(ast, o) == last
        {
            return Some(o);
        }
        if direct.is_empty() {
            return None;
        }
        let all = self.cx.all_overridden(m);
        let mut by_class: FxHashMap<DeclId, DeclId> = FxHashMap::default();
        for &o in all.iter() {
            by_class.entry(parent(ast, o)).or_insert(o);
        }
        bases.list.iter().rev().find_map(|b| by_class.get(b).copied())
    }
}

/// clang `ItaniumVTableContext::getVirtualBaseOffsetOffset(rd, vbase)`.
pub(crate) fn vbase_offset_offset(
    cx: &mut Classes<'_>,
    rd: DeclId,
    vbase: DeclId,
) -> Result<i64, VTableError> {
    let table = if let Some(t) = cx.vbase_offset_offsets.get(&rd) {
        Rc::clone(t)
    } else {
        let r = own_offsets(cx, rd, rd, false)?;
        let t = Rc::new(r.vbase_offset_offsets);
        cx.vbase_offset_offsets.insert(rd, Rc::clone(&t));
        t
    };
    table.iter().find(|(d, _)| *d == vbase).map(|&(_, o)| o).ok_or(VTableError::Incomplete)
}
