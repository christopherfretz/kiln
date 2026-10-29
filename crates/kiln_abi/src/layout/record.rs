//! The Itanium C++ ABI §2.4 class layout algorithm (clang
//! `ItaniumRecordLayoutBuilder`: `InitializeLayout`, `DeterminePrimaryBase`,
//! `LayoutNonVirtualBases`, `LayoutBase`, `LayoutField`, `LayoutBitField`,
//! `LayoutWideBitField`, `LayoutVirtualBases`,
//! `AddPrimaryVirtualBaseOffsets`, `FinishLayout`). All quantities are in
//! bits unless named `_bytes`.
//!
//! Dynamic classes (spec/abi.md §8.5): the primary base (II.1) or else
//! the vptr goes at offset 0, then the non-virtual bases and fields as
//! for any class, then `nvsize`/`nvalign` are taken, then the virtual
//! bases (III) are placed past `dsize` in inheritance-graph order. A
//! virtual base that is the primary base of some base subobject shares
//! that subobject's offset.

use kiln_ast::types::{BuiltinType, RecordLayout, TypeView};
use kiln_ast::{Ast, DeclId, QualType};
use kiln_source::fx::{FxHashMap, FxHashSet};

use super::base_info::{self, BaseInfos};
use super::empty::EmptySubobjectMap;
use super::props::{self, BaseDesc, ClassDesc, FieldDesc};
use super::{FieldTarget, Info, Layouter, RecordFacts, align_to};

/// What the algorithm needs to know about one field, gathered before
/// placement (so placement only reads the session).
#[derive(Clone, Copy, Debug)]
struct FieldIn {
    desc: FieldDesc,
    /// `getTypeInfo(field type)`.
    info: Info,
    /// `FieldDecl::getMaxAlignment()` (bits, 0 = none).
    max_align: u64,
    /// Incomplete array (flexible array member): occupies no space.
    incomplete_array: bool,
    /// The class the field's type designates directly
    /// (`getAsCXXRecordDecl`), first declaration.
    class: Option<DeclId>,
    target: FieldTarget,
}

/// Running state of the builder (clang member names in comments).
#[derive(Debug)]
struct Builder {
    is_union: bool,
    packed: bool,
    /// `Size`.
    size: u64,
    /// `DataSize`.
    data_size: u64,
    /// `UnfilledBitsInLastUnit`.
    unfilled: u64,
    /// `PaddedFieldSize`.
    padded_field_size: u64,
    /// `Alignment`, in bits (a multiple of 8).
    align: u64,
    /// `MaxFieldAlignment` (`#pragma pack`), bits; 0 = none.
    max_field_align: u64,
    offsets: Vec<u64>,
    /// `Bases`: direct non-virtual bases with their byte offsets, in
    /// declaration order.
    bases: Vec<(DeclId, u64)>,
}

impl Builder {
    /// clang `UpdateAlignment` (alignments below a byte are ignored).
    fn update_align(&mut self, bits: u64) {
        let bytes_as_bits = bits / 8 * 8;
        self.align = self.align.max(bytes_as_bits);
    }

    /// `min(a, MaxFieldAlignment)` when a `#pragma pack` is in effect.
    fn cap(&self, a: u64) -> u64 {
        if self.max_field_align == 0 { a } else { a.min(self.max_field_align) }
    }
}

/// The field target of a field of type `q` in the empty-subobject map
/// (clang `CanPlaceFieldSubobjectAtOffset`: a class, or a constant array
/// whose base element is a class).
fn field_target(ast: &Ast, q: QualType) -> FieldTarget {
    let mut v = ast.types.canonical_view(q);
    let mut count: u64 = 1;
    let mut is_array = false;
    for _ in 0..1024 {
        match v {
            TypeView::ConstantArray(a) => {
                is_array = true;
                count = count.saturating_mul(a.size);
                v = ast.types.canonical_view(a.elem);
            }
            TypeView::QualifiedArray { base, .. } => {
                v = ast.types.canonical_view(QualType::unqual(base));
            }
            TypeView::Record(d) if is_array => return FieldTarget::Array(ast.first(d), count),
            TypeView::Record(d) => return FieldTarget::Record(ast.first(d)),
            _ => return FieldTarget::None,
        }
    }
    FieldTarget::None
}

fn is_incomplete_array(ast: &Ast, q: QualType) -> bool {
    matches!(ast.types.canonical_view(q), TypeView::IncompleteArray(_))
}

/// Lays out the class definition `def` (its contained classes are
/// already in the session).
pub(super) fn layout_record(lay: &mut Layouter<'_>, def: DeclId) -> RecordFacts {
    let ast = lay.ast();
    let target = lay.target();
    let class: ClassDesc = props::class_desc(ast, def);
    let descs = props::fields(ast, def);

    // Gather per-field type facts (may lay out further classes reached
    // only through sugar).
    let mut ins = Vec::with_capacity(descs.len());
    for d in &descs {
        let info = lay.type_info(d.ty);
        let max_align = props::max_align_attr(ast, d.decl, target, &mut |q| lay.type_info(q).align);
        ins.push(FieldIn {
            desc: *d,
            info,
            max_align,
            incomplete_array: is_incomplete_array(ast, d.ty),
            class: props::as_record(ast, d.ty),
            target: if d.bit_width.is_some() { FieldTarget::None } else { field_target(ast, d.ty) },
        });
    }
    let record_align_attr =
        props::max_align_attr(ast, def, target, &mut |q| lay.type_info(q).align);

    let lay: &Layouter<'_> = lay;
    let facts_of = |r: DeclId| lay.facts(r);
    let bases = props::bases(ast, def);
    // clang `isDynamicClass`: polymorphic ([class.virtual]/1, which a
    // polymorphic base makes the class) or with a virtual base.
    let dynamic = class.dynamic
        || bases.iter().any(|b| {
            b.class.is_some()
                && (b.is_virtual || b.class.and_then(facts_of).is_some_and(|f| f.dynamic))
        });
    let class = ClassDesc { dynamic, ..class };
    let is_pod = props::is_pod(ast, def, class, &descs, &|r| facts_of(r).is_some_and(|f| f.is_pod));
    let is_empty =
        props::is_empty(ast, def, class, &descs, &|r| facts_of(r).is_some_and(|f| f.is_empty));

    // clang `CXXRecordDecl::vbases()` order: each direct base's virtual
    // bases, then the base itself if it is virtual; each class once.
    let mut vlist: Vec<DeclId> = Vec::new();
    for b in &bases {
        let Some(c) = b.class else { continue };
        if let Some(f) = facts_of(c) {
            for &(v, _) in &f.layout.vbases {
                if !vlist.contains(&v) {
                    vlist.push(v);
                }
            }
        }
        if b.is_virtual && !vlist.contains(&c) {
            vlist.push(c);
        }
    }
    // clang `getIndirectPrimaryBases`: the virtual primary bases of the
    // bases with virtual bases, transitively.
    let mut indirect_primaries: FxHashSet<DeclId> = FxHashSet::default();
    if !vlist.is_empty() {
        for b in &bases {
            if let Some(f) = b.class.and_then(facts_of)
                && !f.layout.vbases.is_empty()
            {
                indirect_primaries.extend(f.vprimaries.iter().copied());
            }
        }
    }
    let primary = if dynamic {
        determine_primary_base(lay, &bases, !vlist.is_empty(), &indirect_primaries)
    } else {
        None
    };
    let mut vprimaries: Vec<DeclId> = indirect_primaries.iter().copied().collect();
    if let Some((p, true)) = primary
        && !vprimaries.contains(&p)
    {
        vprimaries.push(p);
    }
    vprimaries.sort_unstable();

    // clang `EmptySubobjectMap::ComputeEmptySubobjectSizes`: every direct
    // base (virtual ones included) and every field of class or array of
    // class type.
    let empty_size =
        |r: DeclId| facts_of(r).map(|f| if f.is_empty { f.layout.size } else { f.largest_empty });
    let largest_empty = bases
        .iter()
        .filter_map(|b| b.class)
        .chain(descs.iter().filter_map(|d| props::base_element_record(ast, d.ty)))
        .filter_map(empty_size)
        .max()
        .unwrap_or(0);

    // InitializeLayout.
    let mut b = Builder {
        is_union: class.is_union,
        packed: class.packed,
        size: 0,
        data_size: 0,
        unfilled: 0,
        padded_field_size: 0,
        align: 8,
        max_field_align: class.max_field_align,
        offsets: Vec::with_capacity(ins.len()),
        bases: Vec::new(),
    };
    b.update_align(record_align_attr);

    let mut empty_map = EmptySubobjectMap::new(largest_empty);
    let mut infos = base_info::compute(lay, &bases);
    let mut v =
        VBases { offsets: FxHashMap::default(), visited: FxHashSet::default(), indirect_primaries };

    // LayoutNonVirtualBases: the primary base or the vptr first.
    let mut base_offsets: FxHashMap<DeclId, u64> = FxHashMap::default();
    match primary {
        Some((p, true)) => {
            if let Some(&i) = infos.virtuals.get(&p) {
                // Steal it from the base that claimed it.
                infos.node_mut(i).derived = None;
                v.indirect_primaries.insert(p);
                v.visited.insert(p);
                layout_virtual_base(&mut b, lay, &mut empty_map, &infos, &mut v, i);
            }
        }
        Some((p, false)) => {
            if let Some(&i) = infos.non_virtuals.get(&p) {
                let off = layout_base(&mut b, lay, &mut empty_map, &infos, i);
                base_offsets.insert(p, off);
                add_primary_vbase_offsets(lay, &infos, &mut v, i, off);
            }
        }
        None if dynamic => {
            // clang `EnsureVTablePointerAlignment`.
            let ptr = lay.target().pointer_size * 8;
            let mut align = if b.packed { 8 } else { ptr };
            align = b.cap(align);
            b.size = align_to(b.size, align);
            b.update_align(align);
            b.size = b.size.saturating_add(ptr);
            b.data_size = b.size;
        }
        None => {}
    }
    for base in &bases {
        let Some(c) = base.class else { continue };
        if base.is_virtual || primary == Some((c, false)) {
            continue;
        }
        if let Some(&i) = infos.non_virtuals.get(&c) {
            let off = layout_base(&mut b, lay, &mut empty_map, &infos, i);
            base_offsets.insert(c, off);
            add_primary_vbase_offsets(lay, &infos, &mut v, i, off);
        }
    }
    // Direct non-virtual bases in declaration order.
    for base in &bases {
        if let Some(c) = base.class
            && !base.is_virtual
            && let Some(&off) = base_offsets.get(&c)
        {
            b.bases.push((c, off));
        }
    }

    for f in &ins {
        match f.desc.bit_width {
            Some(w) => layout_bit_field(&mut b, lay, f, w),
            None => layout_field(&mut b, lay, &mut empty_map, f),
        }
    }

    // Layout(): the non-virtual size is taken before the virtual bases.
    let nv_size_bits = align_to(b.size, 8);
    let nv_align_bits = b.align;

    if !vlist.is_empty() {
        layout_virtual_bases(&mut b, lay, &mut empty_map, &infos, &mut v, &bases, primary);
        // Defensive: every virtual base has an offset by now.
        for &c in &vlist {
            if !v.offsets.contains_key(&c)
                && let Some(&i) = infos.virtuals.get(&c)
            {
                layout_virtual_base(&mut b, lay, &mut empty_map, &infos, &mut v, i);
            }
        }
    }

    // FinishLayout.
    if b.size == 0 && is_empty {
        b.size = 8;
    }
    b.size = b.size.max(b.padded_field_size);
    b.size = align_to(b.size, b.align);

    let size = b.size / 8;
    let (data_size, nv_size) =
        if is_pod { (size, size) } else { (b.data_size / 8, nv_size_bits / 8) };
    let align = u32::try_from(b.align / 8).unwrap_or(u32::MAX);
    let nv_align = u32::try_from(nv_align_bits / 8).unwrap_or(u32::MAX);
    let vbases: Vec<(DeclId, u64)> =
        vlist.iter().map(|&c| (c, v.offsets.get(&c).copied().unwrap_or(0))).collect();
    RecordFacts {
        layout: RecordLayout {
            size,
            align,
            preferred_align: align,
            data_size,
            nv_size,
            nv_align,
            fields: b.offsets.into_boxed_slice(),
            bases: b.bases.into_boxed_slice(),
            vbases: vbases.into_boxed_slice(),
            vptr: (dynamic && primary.is_none()).then_some(0),
            primary_base: primary,
        },
        is_pod,
        is_empty,
        packed: class.packed,
        largest_empty,
        field_targets: ins.iter().map(|f| f.target).collect(),
        dynamic,
        vprimaries: vprimaries.into_boxed_slice(),
    }
}

/// Is `class` nearly empty (Itanium §1.1: dynamic, and its `nvsize` is
/// one pointer — just the vptr)?
fn nearly_empty(lay: &Layouter<'_>, class: DeclId) -> bool {
    lay.facts(class).is_some_and(|f| f.dynamic && f.layout.nv_size == lay.target().pointer_size)
}

/// clang `DeterminePrimaryBase` (Itanium §2.4 II.1) for a dynamic class
/// with direct bases `bases`: the first non-virtual dynamic base; else
/// the first nearly-empty virtual base, in a preorder walk of the base
/// graph, that is not an indirect primary base; else the first
/// nearly-empty virtual base that is one.
fn determine_primary_base(
    lay: &Layouter<'_>,
    bases: &[BaseDesc],
    has_vbases: bool,
    indirect_primaries: &FxHashSet<DeclId>,
) -> Option<(DeclId, bool)> {
    for b in bases {
        if let Some(c) = b.class
            && !b.is_virtual
            && lay.facts(c).is_some_and(|f| f.dynamic)
        {
            return Some((c, false));
        }
    }
    if !has_vbases {
        return None;
    }
    // clang `SelectPrimaryVBase`: a preorder walk over base-specifiers.
    // Walking a class's bases a second time finds nothing new, so each
    // class is expanded once.
    let mut first_nearly_empty = None;
    let mut expanded: FxHashSet<DeclId> = FxHashSet::default();
    let mut stack: Vec<BaseDesc> = bases.iter().rev().copied().collect();
    while let Some(b) = stack.pop() {
        let Some(c) = b.class else { continue };
        if b.is_virtual && nearly_empty(lay, c) {
            if !indirect_primaries.contains(&c) {
                return Some((c, true));
            }
            first_nearly_empty.get_or_insert(c);
        }
        if expanded.insert(c) {
            stack.extend(base_info::class_bases(lay, c).into_iter().rev());
        }
    }
    first_nearly_empty.map(|c| (c, true))
}

/// Virtual-base placement state.
struct VBases {
    /// clang `VBases`: byte offsets of the virtual bases placed so far.
    offsets: FxHashMap<DeclId, u64>,
    /// clang `VisitedVirtualBases`.
    visited: FxHashSet<DeclId>,
    /// clang `IndirectPrimaryBases`.
    indirect_primaries: FxHashSet<DeclId>,
}

/// clang `LayoutVirtualBase`: places the virtual base `info` like any
/// base and records the offsets of the virtual primary bases it brings.
fn layout_virtual_base(
    b: &mut Builder,
    lay: &Layouter<'_>,
    map: &mut EmptySubobjectMap,
    infos: &BaseInfos,
    v: &mut VBases,
    info: u32,
) {
    let off = layout_base(b, lay, map, infos, info);
    v.offsets.entry(infos.node(info).class).or_insert(off);
    add_primary_vbase_offsets(lay, infos, v, info, off);
}

/// clang `AddPrimaryVirtualBaseOffsets`: a base subobject at `off` puts
/// the virtual primary base it claimed at `off` too, recursively through
/// its non-virtual bases.
fn add_primary_vbase_offsets(
    lay: &Layouter<'_>,
    infos: &BaseInfos,
    v: &mut VBases,
    info: u32,
    off: u64,
) {
    let mut stack = vec![(info, off)];
    while let Some((i, off)) = stack.pop() {
        let n = infos.node(i);
        let Some(facts) = lay.facts(n.class) else { continue };
        if facts.layout.vbases.is_empty() {
            continue;
        }
        if let Some(p) = n.primary_vbase
            && infos.node(p).derived == Some(i)
        {
            v.offsets.entry(infos.node(p).class).or_insert(off);
            stack.push((p, off));
        }
        for &c in &n.bases {
            let child = infos.node(c);
            if child.is_virtual {
                continue;
            }
            let bo =
                facts.layout.bases.iter().find(|(d, _)| *d == child.class).map_or(0, |&(_, o)| o);
            stack.push((c, off.saturating_add(bo)));
        }
    }
}

/// clang `LayoutVirtualBases` (Itanium §2.4 III): a preorder walk over
/// the base-specifiers of the class and, through bases with virtual
/// bases, of its bases; each virtual base that is neither the primary
/// base of the class naming it nor an indirect primary base is placed at
/// its first encounter. Walking a class's bases a second time places
/// nothing, so each class is expanded once.
fn layout_virtual_bases(
    b: &mut Builder,
    lay: &Layouter<'_>,
    map: &mut EmptySubobjectMap,
    infos: &BaseInfos,
    v: &mut VBases,
    bases: &[BaseDesc],
    primary: Option<(DeclId, bool)>,
) {
    let mut expanded: FxHashSet<DeclId> = FxHashSet::default();
    let mut stack: Vec<(Option<(DeclId, bool)>, BaseDesc)> =
        bases.iter().rev().map(|&d| (primary, d)).collect();
    while let Some((owner_primary, d)) = stack.pop() {
        let Some(c) = d.class else { continue };
        if d.is_virtual
            && owner_primary != Some((c, true))
            && !v.indirect_primaries.contains(&c)
            && v.visited.insert(c)
            && let Some(&i) = infos.virtuals.get(&c)
        {
            layout_virtual_base(b, lay, map, infos, v, i);
        }
        let Some(f) = lay.facts(c) else { continue };
        if f.layout.vbases.is_empty() || !expanded.insert(c) {
            continue;
        }
        let pb = f.layout.primary_base;
        stack.extend(base_info::class_bases(lay, c).into_iter().rev().map(|d| (pb, d)));
    }
}

/// clang `LayoutBase` for the base subobject `info` (Itanium §2.4
/// II.2, III), returning its byte offset: an empty base goes at offset 0 unless another empty subobject
/// of the same type is already there; any other base goes at `dsize`
/// rounded up to its `nvalign`, moving up by `nvalign` while an empty
/// subobject conflicts. The base adds its `nvsize` to `dsize`, so later
/// members reuse its tail padding when it is not POD for layout.
fn layout_base(
    b: &mut Builder,
    lay: &Layouter<'_>,
    map: &mut EmptySubobjectMap,
    infos: &BaseInfos,
    info: u32,
) -> u64 {
    let class = infos.node(info).class;
    let Some(facts) = lay.facts(class) else { return 0 };
    let base_size = facts.layout.size.saturating_mul(8);
    let nv_size = facts.layout.nv_size.saturating_mul(8);
    // `packed` does not apply to bases (clang >= 7 outside PS/AIX).
    let base_align = u64::from(facts.layout.nv_align).saturating_mul(8);
    if facts.is_empty && map.can_place_base(lay, infos, info, 0) {
        b.size = b.size.max(base_size);
        b.update_align(base_align);
        return 0;
    }
    let base_align = b.cap(base_align).max(8);
    let mut off = align_to(b.data_size, base_align);
    // Terminates: past the last empty subobject every offset fits.
    while !map.can_place_base(lay, infos, info, off / 8) {
        off = off.saturating_add(base_align);
    }
    if facts.is_empty {
        b.size = b.size.max(off.saturating_add(base_size));
    } else {
        b.data_size = off.saturating_add(nv_size);
        b.size = b.size.max(b.data_size);
    }
    b.update_align(base_align);
    off / 8
}

/// clang `LayoutField` for a non-bit-field member.
fn layout_field(b: &mut Builder, lay: &Layouter<'_>, map: &mut EmptySubobjectMap, f: &FieldIn) {
    let class_facts = f.class.and_then(|c| lay.facts(c));
    let potentially_overlapping = f.desc.nua && f.class.is_some();
    let overlapping_empty = potentially_overlapping && class_facts.is_some_and(|c| c.is_empty);

    let mut off = if b.is_union || overlapping_empty { 0 } else { b.data_size };
    b.unfilled = 0;

    let field_size = if f.incomplete_array { 0 } else { f.info.width };
    let mut effective = field_size;
    if potentially_overlapping && !f.incomplete_array {
        effective = class_facts.map_or(0, |c| c.layout.nv_size.max(c.layout.data_size) * 8);
    }

    let field_packed = (b.packed
        && (f.class.is_none() || class_facts.is_some_and(|c| c.is_pod || c.packed)))
        || f.desc.packed;
    let attr = f.max_align / 8 * 8;
    // The maximum field alignment (`#pragma pack`) overrides even the
    // field's own `aligned` attribute.
    let field_align = if field_packed { b.cap(attr.max(8)) } else { b.cap(f.info.align.max(attr)) };

    off = align_to(off, field_align);
    if !b.is_union {
        // Terminates: past the last empty subobject every offset fits.
        while !map.can_place_field(lay, f.target, off / 8, f.desc.nua) {
            off = if off == 0 && b.data_size != 0 {
                align_to(b.data_size, field_align)
            } else {
                off.saturating_add(field_align)
            };
        }
    }
    b.offsets.push(off);

    if overlapping_empty {
        b.size = b.size.max(off.saturating_add(field_size));
    } else {
        b.data_size =
            if b.is_union { b.data_size.max(effective) } else { off.saturating_add(effective) };
        b.padded_field_size = b.padded_field_size.max(off.saturating_add(field_size));
        b.size = b.size.max(b.data_size);
    }
    b.update_align(field_align);
}

/// clang `LayoutBitField` (System V bit-field allocation, Itanium ABI
/// §2.4 II.3.a).
fn layout_bit_field(b: &mut Builder, lay: &Layouter<'_>, f: &FieldIn, width: u64) {
    let storage_unit = f.info.width;
    if width > storage_unit {
        layout_wide_bit_field(b, lay, width);
        return;
    }
    let field_packed = b.packed || f.desc.packed;
    let mut off = if b.is_union { 0 } else { b.data_size - b.unfilled };
    let mut field_align = f.info.align.max(1);
    if field_packed && width != 0 {
        field_align = 1;
    }
    let mut unpacked_align = f.info.align.max(1);
    let explicit = f.max_align;
    if explicit != 0 {
        field_align = field_align.max(explicit);
        unpacked_align = unpacked_align.max(explicit);
    }
    // A `#pragma pack` overrides even `aligned`, for non-zero widths.
    let mfa = b.max_field_align;
    if mfa != 0 && width != 0 {
        unpacked_align = unpacked_align.min(mfa);
        field_align = if field_packed { unpacked_align } else { field_align.min(mfa) };
    }
    // Any `#pragma pack` suppresses the padding that keeps a bit-field
    // within one aligned unit.
    let allow_padding = mfa == 0;
    if width == 0 || (allow_padding && (off % field_align) + width > storage_unit) {
        off = align_to(off, field_align);
    } else if explicit != 0 && (mfa == 0 || explicit <= mfa) {
        off = align_to(off, explicit);
    }
    b.offsets.push(off);

    if !lay.target().unnamed_bitfield_affects_align && !f.desc.named {
        field_align = 1;
    }
    if b.is_union {
        b.data_size = b.data_size.max(align_to(width, 8));
    } else {
        let end = off.saturating_add(width);
        b.data_size = align_to(end, 8);
        b.unfilled = b.data_size - end;
    }
    b.size = b.size.max(b.data_size);
    b.update_align(field_align);
}

/// clang `LayoutWideBitField`: a bit-field wider than its type is
/// allocated with the alignment of the largest unsigned integer type
/// T' with `sizeof(T') * 8 <= width` (Itanium ABI §2.4 II.3.a.i).
fn layout_wide_bit_field(b: &mut Builder, lay: &Layouter<'_>, width: u64) {
    let target = lay.target();
    let mut t_align = 8;
    for t in [
        BuiltinType::UChar,
        BuiltinType::UShort,
        BuiltinType::UInt,
        BuiltinType::ULong,
        BuiltinType::ULongLong,
    ] {
        let (size, align) = target.builtin_size_align(t);
        if size * 8 > width {
            break;
        }
        t_align = align * 8;
    }
    b.unfilled = 0;
    let off;
    if b.is_union {
        b.data_size = b.data_size.max(align_to(width, 8));
        off = 0;
    } else {
        off = align_to(b.data_size, t_align);
        let end = off.saturating_add(width);
        b.data_size = align_to(end, 8);
        b.unfilled = b.data_size - end;
    }
    b.offsets.push(off);
    b.size = b.size.max(b.data_size);
    b.update_align(t_align);
}
