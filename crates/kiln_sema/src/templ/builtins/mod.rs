//! Compiler builtins (spec/sema-templates.md §9): the type-trait
//! predicates (§9.1, [`traits`]), the type transforms (§9.2,
//! [`transforms`]), the builtin templates (§9.3, [`builtin_templates`])
//! and the staging `TABLE` that `__has_builtin` answers from
//! ([`table`]).

mod builtin_templates;
pub mod table;
mod traits;
mod transforms;

pub(crate) use builtin_templates::declare_builtin_templates;
