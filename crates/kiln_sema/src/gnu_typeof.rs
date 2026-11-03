//! GNU `typeof` (spec/sema.md §5.8): `__typeof`/`__typeof__` (and
//! `typeof` under `-std=gnu++NN`) name the type of an unevaluated
//! expression, as written: unlike `decltype` an lvalue does not add a
//! reference and an id-expression gives the expression's type (clang
//! `ActOnTypeofExpression` / `getTypeOfExprType`). `__typeof_unqual`
//! (`__typeof_unqual__`) is the C23 `typeof_unqual`: the same type
//! without its cv-qualifiers.

use kiln_ast::expr::ExprDeps;
use kiln_ast::{ExprId, QualType};
use kiln_source::Loc;

use crate::Sema;

impl Sema<'_, '_> {
    /// `__typeof ( type-id )`: sugar over the type (`unqual`:
    /// `__typeof_unqual`, canonically without cv-qualifiers).
    pub fn act_on_typeof_type(&mut self, ty: QualType, unqual: bool) -> QualType {
        self.ast.types.typeof_type(ty, unqual)
    }

    /// `__typeof ( expression )`: sugar over the operand's type (its
    /// cv-qualifiers kept, no reference), or `None` when the operand is
    /// erroneous or an unresolved overload set. A type-dependent operand
    /// makes the type dependent, canonically keyed on the expression's
    /// profile (clang's `DependentTypeOfExprType`); otherwise the
    /// canonical type is the operand's.
    pub fn act_on_typeof_expr(&mut self, e: ExprId, _loc: Loc, unqual: bool) -> Option<QualType> {
        let e = self.resolve_single_overload_candidate(e);
        if self.is_err_expr(e) || self.check_overload_use(e) {
            return None;
        }
        let ty = self.ety(e);
        if self.ast.exprs.get(e).deps().contains(ExprDeps::TYPE) {
            let p = self.dependent_profile(e);
            return Some(self.ast.types.typeof_expr(e, QualType::DEPENDENT, unqual, Some(p)));
        }
        Some(self.ast.types.typeof_expr(e, ty, unqual, None))
    }
}
