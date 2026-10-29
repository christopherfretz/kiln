//! Itanium C++ ABI vtables (§2.5–§2.7): vtable groups, construction
//! vtables, VTTs and the `-fdump-vtable-layouts` printer
//! (spec/abi.md §8.5, spec/sema-class.md §3.5, §6.4).
//!
//! The builders are ports of clang 19's `ItaniumVTableBuilder`,
//! `VCallAndVBaseOffsetBuilder`, `FinalOverriders` and `VTTBuilder`
//! (`clang/lib/AST/VTableBuilder.cpp`, `VTTBuilder.cpp`), function by
//! function, so the components, address points, thunks and method
//! indices are clang's; [`dump_vtable_layout`] prints them as
//! `-fdump-vtable-layouts` does. They read the class facts of
//! [`crate::layout`] (primary bases, base offsets) and the AST's
//! override lists (`ast.decls.cold.overrides`, written by sema).
//!
//! Every walk is bounded: a chain of dynamic classes deeper than
//! [`MAX_DYNAMIC_DEPTH`](classes::MAX_DYNAMIC_DEPTH), more than
//! [`MAX_SUBOBJECTS`](crate::layout::MAX_SUBOBJECTS) base paths or more
//! than 2^26 builder steps answers [`VTableError::TooComplex`]. Nothing
//! here panics.

mod builder;
mod classes;
mod overriders;
mod print;
mod vtt;

pub use kiln_ast::class::{
    Subobject, SubobjectId, Thunk, VTableComponent, VTableLayout, VttComponent,
};
use kiln_ast::{Ast, DeclId};
use kiln_source::fx::FxHashMap;

use crate::layout::Layouter;
use crate::target::Target;
use builder::Built;
use classes::Classes;

/// Why no vtable layout was produced.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum VTableError {
    /// The class is not dynamic (no virtual functions or virtual bases):
    /// it has no vtable.
    NotDynamic,
    /// The class is incomplete or invalid: no definition, a virtual
    /// function without a unique final overrider, a missing override
    /// list entry, or a subobject that does not exist.
    Incomplete,
    /// Not implemented (reserved; no current path answers it).
    Unsupported,
    /// The hierarchy exceeds the builders' bounds (see the module
    /// documentation).
    TooComplex,
}

fn session<'a>(ast: &'a Ast, target: &'a Target) -> Classes<'a> {
    Classes::new(Layouter::new(ast, target))
}

/// Checks that `record` is a complete dynamic class within bounds and
/// returns its first declaration.
fn dynamic_class(cx: &mut Classes<'_>, record: DeclId) -> Result<DeclId, VTableError> {
    let ast = cx.ast();
    let first = ast.first(record);
    if ast.definition(first).is_none() {
        return Err(VTableError::Incomplete);
    }
    cx.check_depth(first)?;
    if !cx.get(first).dynamic {
        return Err(VTableError::NotDynamic);
    }
    Ok(first)
}

/// `(class, offset)` of each subobject of `class` → its [`SubobjectId`]
/// (the first, in numbering order).
fn subobject_ids(cx: &mut Classes<'_>, class: DeclId) -> FxHashMap<(DeclId, i64), SubobjectId> {
    let mut map = FxHashMap::default();
    for (i, (s, off)) in cx.lay.subobjects(class).into_iter().enumerate() {
        let Ok(id) = u16::try_from(i) else { break };
        let off = i64::try_from(off).unwrap_or(i64::MAX);
        map.entry((s.class, off)).or_insert(SubobjectId(id));
    }
    map
}

fn complete_built(cx: &mut Classes<'_>, record: DeclId) -> Result<Built, VTableError> {
    let first = dynamic_class(cx, record)?;
    builder::build(cx, first, 0, false, first)
}

/// The construction group of subobject `base` of `most_derived`
/// (`COMPLETE` gives the complete object's own group).
fn construction_built(
    cx: &mut Classes<'_>,
    most_derived: DeclId,
    base: SubobjectId,
) -> Result<Built, VTableError> {
    let md = dynamic_class(cx, most_derived)?;
    if base == SubobjectId::COMPLETE {
        return builder::build(cx, md, 0, false, md);
    }
    let subs = cx.lay.subobjects(md);
    let &(sub, off) = subs.get(base.index()).ok_or(VTableError::Incomplete)?;
    if !cx.get(sub.class).dynamic {
        return Err(VTableError::NotDynamic);
    }
    let off = i64::try_from(off).map_err(|_| VTableError::TooComplex)?;
    builder::build(cx, sub.class, off, sub.is_virtual, md)
}

fn to_layout(cx: &mut Classes<'_>, b: Built) -> VTableLayout {
    let ids = subobject_ids(cx, b.layout_class);
    let mut address_points: Vec<(SubobjectId, u32)> =
        b.address_points.iter().filter_map(|(k, ap)| Some((*ids.get(k)?, *ap))).collect();
    address_points.sort_unstable();
    let thunks: Vec<(u32, Thunk)> =
        b.slot_thunks.iter().filter(|(_, t)| !t.is_empty()).map(|(&i, &t)| (i, t)).collect();
    let mut method_thunks: Vec<(DeclId, Thunk)> =
        b.thunks.iter().flat_map(|(m, list)| list.iter().map(move |&t| (*m, t))).collect();
    method_thunks.sort_by_key(|&(m, _)| m);
    let mut method_indices = b.method_indices;
    method_indices.sort_unstable();
    VTableLayout {
        components: b.components,
        vtable_starts: b.vtable_starts,
        address_points,
        thunks,
        method_thunks,
        vbase_offset_offsets: b.vbase_offset_offsets,
        method_indices,
    }
}

/// The vtable group of dynamic class `record`: its primary vtable and the
/// secondary vtables of its non-primary bases, with address points,
/// thunks, vbase-offset offsets and method indices (Itanium §2.5).
pub fn vtable_layout(
    ast: &Ast,
    record: DeclId,
    target: &Target,
) -> Result<VTableLayout, VTableError> {
    let mut cx = session(ast, target);
    let b = complete_built(&mut cx, record)?;
    Ok(to_layout(&mut cx, b))
}

/// [`vtable_layout`] cached in `ast.vtables` (insert-only, keyed by the
/// first declaration), returning the cached layout.
pub fn ensure_vtable<'a>(
    ast: &'a mut Ast,
    record: DeclId,
    target: &Target,
) -> Result<&'a VTableLayout, VTableError> {
    let first = ast.first(record);
    if ast.vtables.get(first).is_none() {
        let layout = vtable_layout(ast, first, target)?;
        ast.vtables.insert(first, layout);
    }
    ast.vtables.get(first).ok_or(VTableError::Incomplete)
}

/// The construction vtable group used while the base subobject `base` of
/// most-derived class `most_derived` is being constructed or destroyed
/// (Itanium §2.6.1). Its address points name subobjects of
/// `most_derived`. `SubobjectId::COMPLETE` gives the complete object's
/// own group ([`vtable_layout`]).
pub fn construction_vtable(
    ast: &Ast,
    most_derived: DeclId,
    base: SubobjectId,
    target: &Target,
) -> Result<VTableLayout, VTableError> {
    let mut cx = session(ast, target);
    let b = construction_built(&mut cx, most_derived, base)?;
    Ok(to_layout(&mut cx, b))
}

/// The vtable groups the VTT of `record` points into, by
/// [`VttComponent::vtable_index`]: index 0 is `COMPLETE` (the class's
/// own group), the others are the subobjects whose construction groups
/// ([`construction_vtable`]) the VTT uses. Empty for a class without
/// virtual bases.
pub fn vtt_vtables(
    ast: &Ast,
    record: DeclId,
    target: &Target,
) -> Result<Vec<SubobjectId>, VTableError> {
    let mut cx = session(ast, target);
    let md = dynamic_class(&mut cx, record)?;
    let plan = vtt::plan(&mut cx, md)?;
    let ids = subobject_ids(&mut cx, md);
    plan.vtables.iter().map(|(b, _)| ids.get(b).copied().ok_or(VTableError::Incomplete)).collect()
}

/// The VTT of `record` (Itanium §2.6.2); empty for a class without
/// virtual bases. Each entry names the vtable group (see
/// [`vtt_vtables`]) and the subobject of `record` whose address point
/// in that group it holds.
pub fn vtt(ast: &Ast, record: DeclId, target: &Target) -> Result<Vec<VttComponent>, VTableError> {
    let mut cx = session(ast, target);
    let md = dynamic_class(&mut cx, record)?;
    let plan = vtt::plan(&mut cx, md)?;
    let ids = subobject_ids(&mut cx, md);
    plan.components
        .iter()
        .map(|&(i, b)| {
            Ok(VttComponent { vtable_index: i, base: *ids.get(&b).ok_or(VTableError::Incomplete)? })
        })
        .collect()
}

/// The `-fdump-vtable-layouts` text of `record` as clang 19 prints it
/// (`Vtable for 'B' (13 entries).`, the vbase-offset, thunk and
/// method-index tables), without clang's leading `Original map` block.
pub fn dump_vtable_layout(
    ast: &Ast,
    record: DeclId,
    target: &Target,
) -> Result<String, VTableError> {
    let mut cx = session(ast, target);
    let b = complete_built(&mut cx, record)?;
    let mut out = String::new();
    print::write_built(&mut cx, &b, &mut out);
    Ok(out)
}

/// The `-fdump-vtable-layouts` text of the construction vtable group of
/// subobject `base` of `most_derived` (`Construction vtable for ('B',
/// 16) in 'D' (N entries).`).
pub fn dump_construction_vtable(
    ast: &Ast,
    most_derived: DeclId,
    base: SubobjectId,
    target: &Target,
) -> Result<String, VTableError> {
    let mut cx = session(ast, target);
    let b = construction_built(&mut cx, most_derived, base)?;
    let mut out = String::new();
    print::write_built(&mut cx, &b, &mut out);
    Ok(out)
}

/// `-fdump-vtable-layouts` for a whole translation unit: the vtable
/// group ([`dump_vtable_group`]) of every dynamic class of
/// [`crate::record_definitions`], in that order. clang instead dumps a
/// class when it emits its vtable (spec/abi.md §8.5); classes kiln cannot
/// lay out ([`VTableError`]) are skipped.
pub fn dump_all_vtable_layouts(ast: &Ast, target: &Target) -> String {
    let mut out = String::new();
    for d in crate::record_definitions(ast) {
        if let Ok(text) = dump_vtable_group(ast, d, target) {
            out.push_str(&text);
        }
    }
    out
}

/// The vtable dump of `record` followed by those of the construction
/// groups its VTT uses, in VTT order (what clang prints when it emits
/// the class's vtable and VTT).
pub fn dump_vtable_group(
    ast: &Ast,
    record: DeclId,
    target: &Target,
) -> Result<String, VTableError> {
    let mut cx = session(ast, target);
    let b = complete_built(&mut cx, record)?;
    let mut out = String::new();
    print::write_built(&mut cx, &b, &mut out);
    let md = b.most_derived;
    let plan = vtt::plan(&mut cx, md)?;
    for &((class, off), is_virtual) in plan.vtables.iter().skip(1) {
        let c = builder::build(&mut cx, class, off, is_virtual, md)?;
        print::write_built(&mut cx, &c, &mut out);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use kiln_ast::decl::TagKind;
    use kiln_ast::names::DeclName;
    use kiln_ast::types::TypeContext;
    use kiln_ast::{Ast, DeclHead};
    use kiln_source::Loc;
    use kiln_source::symbol::Interner;

    use super::*;

    #[test]
    fn non_dynamic_classes_are_rejected_without_caching() {
        let t = crate::target("x86_64-unknown-linux-gnu").expect("target");
        let mut ast = Ast::new(Arc::new(Interner::new()), TypeContext::new(t.types.clone()));
        let h = DeclHead::new(ast.tu, DeclName::EMPTY, Loc::INVALID);
        let r = ast.builder().record(h, TagKind::Struct, None);
        // Forward-declared only: no definition.
        assert_eq!(vtable_layout(&ast, r, &t), Err(VTableError::Incomplete));
        assert_eq!(vtt(&ast, r, &t), Err(VTableError::Incomplete));
        assert_eq!(ensure_vtable(&mut ast, r, &t).err(), Some(VTableError::Incomplete));
        // Defined but not polymorphic.
        ast.builder().begin_record_definition(r).expect("first definition");
        ast.builder().complete_record_definition(r, kiln_ast::decl::DefinitionBits::empty());
        assert_eq!(vtable_layout(&ast, r, &t), Err(VTableError::NotDynamic));
        assert_eq!(vtt(&ast, r, &t), Err(VTableError::NotDynamic));
        assert_eq!(
            construction_vtable(&ast, r, SubobjectId::COMPLETE, &t),
            Err(VTableError::NotDynamic)
        );
        assert_eq!(dump_vtable_layout(&ast, r, &t), Err(VTableError::NotDynamic));
        assert_eq!(ensure_vtable(&mut ast, r, &t).err(), Some(VTableError::NotDynamic));
        assert!(ast.vtables.is_empty());
    }
}
