//! Side effects in unevaluated operands ([expr.context]/1): clang's
//! `-Wunevaluated-expression` ("expression with side effects has no
//! effect in an unevaluated context") on the operands of `sizeof`,
//! `alignof`, `decltype`, `noexcept` and a `typeid` that is not
//! evaluated. Also the possible side effects of an assumption
//! ([dcl.attr.assume]; clang's `-Wassume`).

use kiln_ast::ExprId;
use kiln_ast::attr::AttrKind;
use kiln_ast::expr::{CastKind, ExprDeps, ExprKind, ValueCategory};
use kiln_ast::ids::RawId;
use kiln_ast::types::Quals;
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;

impl Sema<'_, '_> {
    /// Warns when unevaluated operand `e` has side effects, unless it is
    /// instantiation-dependent or this is a template instantiation (as
    /// clang, which checks the pattern instead), at `e`'s location.
    /// [expr.typeid]/3: the evaluated operand `e` of `typeid` (a
    /// polymorphic glvalue) with possible side effects, calls included
    /// (clang `warn_side_effects_typeid`): not in an instantiation, nor
    /// for an instantiation-dependent operand (the definition warns).
    pub(crate) fn check_typeid_side_effects(&mut self, e: ExprId) {
        if !self.inst.stack.is_empty()
            || self.ast.exprs.get(e).deps().intersects(ExprDeps::TYPE_VALUE_INST | ExprDeps::ERROR)
            || !self.has_possible_side_effects(e)
        {
            return;
        }
        let loc = self.expr_loc(e);
        let msg = self.diag(DiagId::sema_side_effects_typeid, loc);
        self.emit(msg);
    }

    pub(crate) fn check_unevaluated_side_effects(&mut self, e: ExprId) {
        if !self.inst.stack.is_empty() || self.is_err_expr(e) || !self.has_side_effects(e) {
            return;
        }
        let loc = self.expr_loc(e);
        let msg = self.diag(DiagId::sema_side_effects_unevaluated_context, loc);
        self.emit(msg);
    }

    /// clang's `Expr::getExprLoc`: an operator's location for unary,
    /// binary and member expressions; a subscript's base's; a member
    /// call's member name; where it begins otherwise.
    pub fn expr_loc(&self, mut e: ExprId) -> Loc {
        for _ in 0..256 {
            let exprs = &self.ast.exprs;
            match exprs.get(e).kind() {
                ExprKind::ArraySubscriptExpr | ExprKind::CXXMemberCallExpr => {
                    e = exprs.children(e, &self.ast.lists)[0];
                }
                ExprKind::UnaryOperator
                | ExprKind::BinaryOperator
                | ExprKind::CompoundAssignOperator
                | ExprKind::MemberExpr
                | ExprKind::CXXRewrittenBinaryOperator => return self.eloc(e),
                ExprKind::CXXOperatorCallExpr => {
                    use kiln_ast::names::OverloadedOperator as O;
                    let op = exprs.operator_call_op(e);
                    if matches!(
                        op,
                        O::PlusPlus
                            | O::MinusMinus
                            | O::Arrow
                            | O::Call
                            | O::Subscript
                            | O::New
                            | O::Delete
                            | O::ArrayNew
                            | O::ArrayDelete
                    ) {
                        return self.begin_loc(e);
                    }
                    return self.eloc(e);
                }
                _ => return self.begin_loc(e),
            }
        }
        self.eloc(e)
    }

    /// clang's `Expr::HasSideEffects(Ctx, /*IncludePossibleEffects=*/false)`:
    /// an increment or decrement, an assignment, a `new`, `delete`,
    /// `throw`, `co_await` or `co_yield`, or an evaluated `typeid`,
    /// anywhere in `e` outside nested unevaluated operands and lambda
    /// bodies. A call adds none of its own; an instantiation-dependent
    /// expression and one written in a macro expansion have none.
    pub(crate) fn has_side_effects(&self, e: ExprId) -> bool {
        let exprs = &self.ast.exprs;
        let dependent = ExprDeps::TYPE | ExprDeps::VALUE | ExprDeps::UNEXPANDED_PACK;
        let mut stack = vec![e];
        while let Some(x) = stack.pop() {
            let n = exprs.get(x);
            // Kiln marks every reference to a declaration in a dependent
            // context instantiation-dependent (spec/sema-templates.md
            // §2.4); clang does not, so only type and value dependence
            // count as dependence here.
            if n.deps().intersects(dependent) || n.loc().is_macro() {
                continue;
            }
            match n.kind() {
                ExprKind::UnaryOperator if exprs.unary_op(x).is_inc_dec() => return true,
                ExprKind::BinaryOperator | ExprKind::CompoundAssignOperator
                    if exprs.binary_op(x).is_assignment() =>
                {
                    return true;
                }
                ExprKind::CXXNewExpr
                | ExprKind::CXXDeleteExpr
                | ExprKind::AtomicExpr
                | ExprKind::VAArgExpr
                | ExprKind::CXXThrowExpr
                | ExprKind::CoawaitExpr
                | ExprKind::DependentCoawaitExpr
                | ExprKind::CoyieldExpr => return true,
                ExprKind::CXXTypeidExpr => {
                    if exprs.typeid_potentially_evaluated(x) {
                        return true;
                    }
                }
                ExprKind::UnaryExprOrTypeTraitExpr
                | ExprKind::CXXNoexceptExpr
                | ExprKind::LambdaExpr
                | ExprKind::RequiresExpr => {}
                _ => stack.extend(exprs.children(x, &self.ast.lists)),
            }
        }
        false
    }

    /// clang's `Expr::HasSideEffects(Ctx, /*IncludePossibleEffects=*/true)`:
    /// [`Self::has_side_effects`] plus what may have effects: a call to a
    /// function that is neither `const` nor `pure`, a volatile read, a
    /// non-trivial constructor, a temporary with a non-trivial destructor,
    /// a `dynamic_cast` to a reference (it may throw), a lambda's capture
    /// initializers, the default arguments and member initializers used,
    /// and any dependent or erroneous node (conservatively). Macro
    /// expansions are not exempt.
    #[allow(clippy::too_many_lines, reason = "one arm per expression kind")]
    pub(crate) fn has_possible_side_effects(&self, e: ExprId) -> bool {
        let exprs = &self.ast.exprs;
        let dependent = ExprDeps::TYPE | ExprDeps::VALUE | ExprDeps::UNEXPANDED_PACK;
        let mut stack = vec![e];
        let mut seen = 0usize;
        while let Some(x) = stack.pop() {
            seen += 1;
            if seen > 1_000_000 {
                return true;
            }
            let n = exprs.get(x);
            if n.deps().intersects(dependent | ExprDeps::ERROR) {
                return true;
            }
            match n.kind() {
                ExprKind::UnaryOperator if exprs.unary_op(x).is_inc_dec() => return true,
                ExprKind::BinaryOperator | ExprKind::CompoundAssignOperator
                    if exprs.binary_op(x).is_assignment() =>
                {
                    return true;
                }
                ExprKind::CXXNewExpr
                | ExprKind::CXXDeleteExpr
                | ExprKind::AtomicExpr
                | ExprKind::CXXThrowExpr
                | ExprKind::CoawaitExpr
                | ExprKind::DependentCoawaitExpr
                | ExprKind::CoyieldExpr
                | ExprKind::CXXBindTemporaryExpr
                | ExprKind::UnresolvedLookupExpr
                | ExprKind::UnresolvedMemberExpr
                | ExprKind::DependentScopeDeclRefExpr
                | ExprKind::CXXDependentScopeMemberExpr
                | ExprKind::CXXUnresolvedConstructExpr
                | ExprKind::PackExpansionExpr
                | ExprKind::SubstNonTypeTemplateParmPackExpr
                | ExprKind::FunctionParmPackExpr
                | ExprKind::CXXFoldExpr
                | ExprKind::RecoveryExpr => return true,
                ExprKind::CXXTypeidExpr => {
                    if exprs.typeid_potentially_evaluated(x) {
                        return true;
                    }
                }
                ExprKind::CallExpr
                | ExprKind::CXXMemberCallExpr
                | ExprKind::CXXOperatorCallExpr
                | ExprKind::UserDefinedLiteral => {
                    let pure = self.callee_decl(x).is_some_and(|f| {
                        self.find_attr(f, AttrKind::CONST).is_some()
                            || self.find_attr(f, AttrKind::PURE).is_some()
                    });
                    if !pure {
                        return true;
                    }
                    stack.extend(exprs.children(x, &self.ast.lists));
                }
                ExprKind::ImplicitCastExpr
                | ExprKind::CStyleCastExpr
                | ExprKind::CXXFunctionalCastExpr
                | ExprKind::CXXStaticCastExpr
                | ExprKind::CXXConstCastExpr
                | ExprKind::CXXReinterpretCastExpr
                | ExprKind::CXXDynamicCastExpr
                | ExprKind::BuiltinBitCastExpr => {
                    let ck = exprs.cast_kind(x);
                    if n.kind() == ExprKind::CXXDynamicCastExpr
                        && n.value_category() != ValueCategory::PRValue
                        && ck == CastKind::Dynamic
                    {
                        return true;
                    }
                    let kids = exprs.children(x, &self.ast.lists);
                    if ck == CastKind::LValueToRValue
                        && kids.first().is_some_and(|&k| {
                            self.ast
                                .types
                                .effective_quals(exprs.get(k).ty())
                                .contains(Quals::VOLATILE)
                        })
                    {
                        return true;
                    }
                    stack.extend(kids);
                }
                ExprKind::CXXConstructExpr
                | ExprKind::CXXTemporaryObjectExpr
                | ExprKind::CXXInheritedCtorInitExpr => {
                    let c = exprs.constructor(x);
                    if !self.ast.decls.function(c).is_some_and(|f| f.spec.is_trivial()) {
                        return true;
                    }
                    stack.extend(exprs.children(x, &self.ast.lists));
                }
                ExprKind::CXXDefaultArgExpr => {
                    let p = exprs.default_arg_param(x);
                    stack.extend(self.ast.decls.param(p).and_then(|p| p.default_arg));
                }
                ExprKind::CXXDefaultInitExpr => {
                    let f = exprs.default_init_field(x);
                    stack.extend(self.ast.decls.field(f).and_then(|f| f.init));
                }
                ExprKind::LambdaExpr => {
                    if let Some(l) = exprs.lambda(x).inits {
                        stack.extend(
                            self.ast.lists.get(l).iter().filter_map(|&r| ExprId::from_raw(r)),
                        );
                    }
                }
                ExprKind::UnaryExprOrTypeTraitExpr
                | ExprKind::CXXNoexceptExpr
                | ExprKind::RequiresExpr => {}
                _ => stack.extend(exprs.children(x, &self.ast.lists)),
            }
        }
        false
    }

    /// The function a call expression calls, if it names one.
    pub(crate) fn callee_decl(&self, c: ExprId) -> Option<kiln_ast::DeclId> {
        let exprs = &self.ast.exprs;
        let mut r = exprs.callee(c);
        for _ in 0..64 {
            match exprs.get(r).kind() {
                ExprKind::ImplicitCastExpr | ExprKind::ParenExpr => {
                    r = *exprs.children(r, &self.ast.lists).first()?;
                }
                ExprKind::DeclRefExpr | ExprKind::MemberExpr => return Some(exprs.decl(r)),
                _ => return None,
            }
        }
        None
    }
}
