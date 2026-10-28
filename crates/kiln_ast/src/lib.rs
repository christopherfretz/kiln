//! The typed AST: types, declarations, expressions, statements, templates; stable dump format.
//!
//! See `docs/ARCHITECTURE.md` for the crate's place in the pipeline. The
//! contract this crate implements is specified in:
//!
//! - `spec/ast.md`: arenas and ids, declarations, expressions, statements,
//!   templates, constant values (`ConstStore`), reflection hooks;
//! - `spec/types.md`: `QualType`, `TypeContext` interning, canonical types,
//!   type kinds, layout cache hooks, type printing;
//! - `spec/ast-dump.md`: the `-fdump-ast` text format (the parser+sema
//!   test contract).
#![forbid(unsafe_code)]
