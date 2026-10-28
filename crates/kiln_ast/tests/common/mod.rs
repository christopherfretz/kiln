//! Shared helpers for the `kiln_ast` integration tests.
#![allow(dead_code, reason = "each test binary uses a subset")]

use std::sync::Arc;

use kiln_ast::types::TypeContext;
use kiln_ast::{Ast, QualType, TypeId};
use kiln_source::symbol::Interner;

/// The one place tests construct a `TypeContext` (WP-042 may replace
/// `Default` with `TypeContext::new(TargetTypes)`).
pub fn type_context() -> TypeContext {
    TypeContext::default()
}

/// A fresh AST with its own interner.
pub fn new_ast() -> Ast {
    Ast::new(Arc::new(Interner::new()), type_context())
}

/// Builtin type ids (spec/types.md §5).
pub const BOOL: u32 = 2;
/// `int`.
pub const INT: u32 = 12;
/// The `Dependent` placeholder.
pub const DEPENDENT: u32 = 29;
/// The error type.
pub const ERROR: u32 = 33;

/// The unqualified builtin type `index`.
pub fn builtin(index: u32) -> QualType {
    QualType::unqual(TypeId::new(index))
}

/// `const`-qualified builtin `index`.
pub fn const_builtin(index: u32) -> QualType {
    QualType::new(TypeId::new(index), 1)
}
