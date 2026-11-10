//! [`TypeNames`] over a [`Sema`]'s AST: how declarations, expressions and
//! values print inside types in diagnostics. The printer lives in
//! `kiln_ast` ([`kiln_ast::diag_names::DiagNames`]) so that codegen's sanitizer
//! type descriptors print with it too (WP-1158).

pub use kiln_ast::diag_names::DiagNames as SemaNames;

use crate::Sema;

/// The diagnostic names of `sema`'s AST, at its `-fclang-compat` level.
pub fn sema_names<'s>(sema: &'s Sema<'_, '_>) -> SemaNames<'s> {
    SemaNames::new(&sema.ast, Some(sema.diags.sm()), sema.clang_compat)
}
