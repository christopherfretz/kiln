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

pub mod annot;
pub mod ast;
pub mod attr;
pub mod builder;
pub mod class;
pub mod consteval_only;
pub mod consts;
pub mod decl;
pub mod decl_extents;
pub mod diag_names;
pub mod dump;
pub mod expr;
pub mod expr_extent;
pub mod fp;
pub mod ids;
pub mod journal;
pub mod lambda;
mod layout_sugar;
pub mod linkage;
pub mod lists;
pub mod lookup;
pub mod mem;
pub mod modules;
pub mod multiversion;
pub mod names;
mod neon_builtins;
pub mod oper_lookup;
pub mod redecl;
pub mod reflect;
pub mod roles;
pub mod sanitize;
pub mod slot;
pub mod stmt;
pub mod strings;
pub mod sve;
mod sve_builtins;
mod sve_checks;
pub mod templates;
pub mod types;
pub mod written_refs;
mod x86_builtins;

pub use ast::Ast;
pub use builder::{AstBuilder, DeclHead};
pub use ids::{
    AnnId, ArgListId, AttrId, BlobId, ConstValueId, DeclId, DmsId, ExprId, LambdaId, ListId, NnsId,
    ProfileId, QualType, RawId, RedeclGroupId, ReflectionId, StmtId, StringLitId, TemplateNameId,
    TemplateParamListId, TypeId,
};
pub use lists::IdLists;
pub use mem::{HeapSize, MemReport, MemRow, VecDims};
