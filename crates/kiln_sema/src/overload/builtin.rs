//! Built-in operator candidates ([over.built]; spec/sema-overload.md §5.3).

use kiln_ast::names::OverloadedOperator as OO;
use kiln_ast::{ExprId, QualType};

use super::OverloadCandidateSet;
use super::oper::OperForm;
use crate::Sema;

impl Sema<'_, '_> {
    /// Adds the built-in candidates of `op` for operands of `types`
    /// ([over.match.oper]/3.3).
    pub(crate) fn add_builtin_candidates(
        &mut self,
        _set: &mut OverloadCandidateSet,
        _op: OO,
        _form: OperForm,
        _ops: &[ExprId],
        _types: &[QualType],
    ) {
    }

    /// The operand conversions of selected built-in candidate `b`
    /// ([over.match.oper]/11); `None` after diagnosing.
    pub(crate) fn builtin_operand_conversions(
        &mut self,
        _set: &OverloadCandidateSet,
        _b: u32,
        _ops: &[ExprId],
    ) -> Option<()> {
        Some(())
    }
}
