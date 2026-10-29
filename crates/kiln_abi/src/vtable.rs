//! Itanium C++ ABI vtables (§2.5–§2.7): vtable groups, construction
//! vtables, VTTs and the `-fdump-vtable-layouts` printer
//! (spec/sema-class.md §3.5, §6.4).
//!
//! WP-066a lands the interface: the data types (stored in
//! `Ast::vtables`, so they live in `kiln_ast::class` and are re-exported
//! here) and the entry points, whose bodies answer
//! [`VTableError::Unsupported`] until WP-078b. Nothing here panics.

pub use kiln_ast::class::{
    Subobject, SubobjectId, Thunk, VTableComponent, VTableLayout, VttComponent,
};
use kiln_ast::{Ast, DeclId};

use crate::target::Target;

/// Why no vtable layout was produced.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum VTableError {
    /// The class is not dynamic (no virtual functions or virtual bases):
    /// it has no vtable.
    NotDynamic,
    /// The class is incomplete or invalid.
    Incomplete,
    /// Not implemented yet (WP-078b).
    Unsupported,
}

/// The vtable group of dynamic class `record`: its primary vtable and the
/// secondary vtables of its non-primary bases, with address points,
/// thunks, vbase-offset offsets and method indices (Itanium §2.5).
pub fn vtable_layout(
    ast: &Ast,
    record: DeclId,
    target: &Target,
) -> Result<VTableLayout, VTableError> {
    let _ = (ast, record, target);
    Err(VTableError::Unsupported)
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
    ast.vtables.get(first).ok_or(VTableError::Unsupported)
}

/// The construction vtable group used while the base subobject `base` of
/// most-derived class `most_derived` is being constructed or destroyed
/// (Itanium §2.6.1; only for classes with virtual bases).
pub fn construction_vtable(
    ast: &Ast,
    most_derived: DeclId,
    base: SubobjectId,
    target: &Target,
) -> Result<VTableLayout, VTableError> {
    let _ = (ast, most_derived, base, target);
    Err(VTableError::Unsupported)
}

/// The VTT of `record` (Itanium §2.6.2); empty for a class without
/// virtual bases.
pub fn vtt(ast: &Ast, record: DeclId, target: &Target) -> Result<Vec<VttComponent>, VTableError> {
    let _ = (ast, record, target);
    Err(VTableError::Unsupported)
}

/// The `-fdump-vtable-layouts` text of `record` as clang 19 prints it
/// (`Vtable for 'B' (13 entries).`, the thunk, method-index and
/// vbase-offset tables).
pub fn dump_vtable_layout(
    ast: &Ast,
    record: DeclId,
    target: &Target,
) -> Result<String, VTableError> {
    let _ = (ast, record, target);
    Err(VTableError::Unsupported)
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
    fn stubs_answer_unsupported_without_caching() {
        let t = crate::target("x86_64-unknown-linux-gnu").expect("target");
        let mut ast = Ast::new(Arc::new(Interner::new()), TypeContext::new(t.types.clone()));
        let h = DeclHead::new(ast.tu, DeclName::EMPTY, Loc::INVALID);
        let r = ast.builder().record(h, TagKind::Struct, None);
        assert_eq!(vtable_layout(&ast, r, &t), Err(VTableError::Unsupported));
        assert_eq!(vtt(&ast, r, &t), Err(VTableError::Unsupported));
        assert_eq!(
            construction_vtable(&ast, r, SubobjectId::COMPLETE, &t),
            Err(VTableError::Unsupported)
        );
        assert_eq!(dump_vtable_layout(&ast, r, &t), Err(VTableError::Unsupported));
        assert_eq!(ensure_vtable(&mut ast, r, &t).err(), Some(VTableError::Unsupported));
        assert!(ast.vtables.is_empty());
    }
}
