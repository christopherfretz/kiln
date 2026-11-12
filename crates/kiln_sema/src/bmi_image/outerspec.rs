//! Explicit specializations of member class templates defined outside
//! their class (spec/modules.md §11.11, "Forms carried by WP-1353").
//!
//! `template <> struct outer::res<void> { ... };` ([temp.expl.spec]/2,
//! `__future_base::_Result<void>` in libstdc++'s `<future>`) is written
//! as the out-of-class partial specialization of WP-1345 is: a `Record`
//! record whose context is the class and whose lexical context is the
//! namespace, with no previous record, listed in `cspecs` as any
//! explicit specialization.

use kiln_ast::DeclId;
use kiln_ast::templates::SpecKind;

use super::Writer;

impl Writer<'_, '_, '_> {
    /// Whether class `d` is an explicit specialization (of a member class
    /// template, when its context is a class).
    pub(super) fn outer_explicit_spec(&self, d: DeclId) -> bool {
        let ast = &self.s.ast;
        ast.spec_info(ast.first(d)).is_some_and(|(_, s)| s.kind == SpecKind::ExplicitSpecialization)
    }
}
