//! PLACEHOLDER from the WP-041 stub commit: owned by WP-042 (types), which
//! replaces this file wholesale (spec/types.md). It provides only the items
//! `ast.rs` and `builder.rs` (WP-041) call, with the signatures of
//! spec/types.md §13, so the crate compiles before WP-042 lands:
//!
//! - `TypeContext: Default` (an `Ast` is created with one; WP-042 may
//!   instead require `TypeContext::new(TargetTypes)` — then
//!   `Ast::new`'s callers pass it in, as they already do);
//! - `TypeContext::flags(TypeId) -> TypeFlags` with `DEPENDENT`,
//!   `INSTANTIATION_DEPENDENT`, `UNEXPANDED_PACK`, `CONTAINS_ERROR`
//!   (expression dependence is computed from them, ast.md §5.1);
//! - `record_type`, `enum_type`, `typedef_type` (one `D` node per
//!   declaration, called by `AstBuilder`);
//! - `HeapSize` for `TypeContext` and `LayoutCache`.
//!
//! The placeholder reports builtin 29 (`Dependent`) as dependent and
//! builtin 33 (`Error`) as containing an error, as the real context must.

use crate::ids::{DeclId, QualType, TypeId};
use crate::mem::{HeapSize, MemReport, MemRow};

/// Placeholder for the type flags of spec/types.md §6.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct TypeFlags(u8);

impl TypeFlags {
    /// Dependent type [temp.dep.type].
    pub const DEPENDENT: TypeFlags = TypeFlags(1);
    /// Mentions a template parameter somewhere.
    pub const INSTANTIATION_DEPENDENT: TypeFlags = TypeFlags(2);
    /// Contains a VLA.
    pub const VARIABLY_MODIFIED: TypeFlags = TypeFlags(4);
    /// Contains an unexpanded parameter pack.
    pub const UNEXPANDED_PACK: TypeFlags = TypeFlags(8);
    /// Built from an error type or a recovery expression.
    pub const CONTAINS_ERROR: TypeFlags = TypeFlags(16);
    /// Contains an undeduced placeholder.
    pub const UNDEDUCED: TypeFlags = TypeFlags(32);
    /// Sugar node.
    pub const SUGAR: TypeFlags = TypeFlags(64);

    /// True if every bit of `other` is set.
    pub const fn contains(self, other: TypeFlags) -> bool {
        self.0 & other.0 == other.0
    }
}

/// Placeholder `TypeContext` (spec/types.md §4, §13).
#[derive(Debug, Default)]
pub struct TypeContext {
    decl_nodes: Vec<(u8, DeclId, Option<QualType>)>,
}

/// First type index after the builtins and their reserved range.
const FIRST_DECL_TYPE: u32 = 64;

impl TypeContext {
    /// The flags of type node `t`.
    pub fn flags(&self, t: TypeId) -> TypeFlags {
        match t.index() {
            29 => TypeFlags(TypeFlags::DEPENDENT.0 | TypeFlags::INSTANTIATION_DEPENDENT.0),
            33 => TypeFlags::CONTAINS_ERROR,
            _ => TypeFlags::default(),
        }
    }

    fn push(&mut self, kind: u8, d: DeclId, under: Option<QualType>) -> TypeId {
        self.decl_nodes.push((kind, d, under));
        TypeId::new(FIRST_DECL_TYPE + self.decl_nodes.len() as u32 - 1)
    }

    /// The `Record` type of a class (called once per class, with its first
    /// declaration).
    pub fn record_type(&mut self, first_decl: DeclId) -> TypeId {
        self.push(0, first_decl, None)
    }

    /// The `Enum` type of an enumeration (once, with its first declaration).
    pub fn enum_type(&mut self, first_decl: DeclId) -> TypeId {
        self.push(1, first_decl, None)
    }

    /// The `Typedef` sugar type of a typedef or alias declaration.
    pub fn typedef_type(&mut self, decl: DeclId, underlying: QualType) -> TypeId {
        self.push(2, decl, Some(underlying))
    }
}

impl HeapSize for TypeContext {
    fn heap_bytes(&self) -> MemReport {
        let mut r = MemReport::new();
        r.push(MemRow::of_vec("nodes", &self.decl_nodes));
        r
    }
}

/// Placeholder for the record layout cache (spec/types.md §11).
#[derive(Debug, Default)]
pub struct LayoutCache {}

impl HeapSize for LayoutCache {
    fn heap_bytes(&self) -> MemReport {
        MemReport::new()
    }
}
