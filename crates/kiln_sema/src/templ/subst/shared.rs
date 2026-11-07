//! Marking the declarations of an expression that an instantiation shares
//! with its pattern (spec/sema.md §5.10).
//!
//! A non-dependent expression is not rebuilt by substitution: the
//! instantiation reuses the pattern's node. Inside the template definition
//! its functions were only *referenced* ([temp.res.general]/6), so the
//! instantiation must odr-use them itself, as clang's `TreeTransform` does by
//! rebuilding the expression (`MarkDeclRefReferenced`, `MarkFunctionReferenced`).
//! The default arguments and default member initializers it uses have
//! their immediate invocations evaluated again, and so are its own
//! immediate invocations that failed at the definition, and the
//! null dereferences the definition warned about are warned about again
//! (spec/sema.md §5.15), and so are the arguments of its variadic calls
//! that the definition did not check (`Sema::check_variadic_args`), and
//! so are its always-non-null comparisons (`Sema::reissue_null_compares`). Unevaluated operands and lambda bodies are not
//! entered.

use kiln_ast::ExprId;
use kiln_ast::decl::DeclKind;
use kiln_ast::expr::{ExprKind, NonOdrUse};
use kiln_source::Loc;

use crate::Sema;

impl Sema<'_, '_> {
    /// Marks the declaration a shared `DeclRefExpr` or `MemberExpr` `x`
    /// names: counted, referenced if a function, and odr-used if a
    /// variable.
    fn reference_shared_decl(&mut self, x: ExprId, kind: ExprKind, loc: Loc) {
        let d = self.ast.exprs.decl(x);
        // clang `TransformDeclRefExpr` marks a kept node
        // referenced, which counts it for
        // `-Wunused-but-set-variable` against the discarded
        // assignments the instantiated statements subtract.
        if kind == ExprKind::DeclRefExpr
            && matches!(self.ast.decls.get(d).kind(), DeclKind::Var | DeclKind::ParmVar)
        {
            self.count_var_ref(d);
        }
        if self.ast.exprs.non_odr_use(x) == NonOdrUse::None && self.ast.decls.function(d).is_some()
        {
            self.mark_function_referenced(d, loc);
        } else if kind == ExprKind::DeclRefExpr && self.ast.decls.get(d).kind() == DeclKind::Var {
            // A variable named in the pattern is odr-used by
            // the instantiation, decided at its full-expression
            // as for a fresh reference ([basic.def.odr]/5).
            self.pending_odr.push(x);
        }
    }

    /// The shared name `x` used again ([`Self::reference_shared_decl`]),
    /// with the expressions among its explicit template arguments pushed
    /// for [`Self::mark_shared_expr_referenced`]'s walk: they are used
    /// again too (`f<&C::g>()` kept from a generic lambda's pattern, whose
    /// `&C::g` was formed in a dependent context; clang's
    /// `TransformDeclRefExpr` transforms them).
    fn reference_shared_name(
        &mut self,
        x: ExprId,
        kind: ExprKind,
        loc: Loc,
        stack: &mut Vec<ExprId>,
    ) {
        self.reference_shared_decl(x, kind, loc);
        let Some(l) = self.ast.exprs.decl_ref_extra(x).and_then(|r| r.template_args) else {
            return;
        };
        for &a in self.ast.types.args(l) {
            if let kiln_ast::types::ArgView::Expr { expr, .. } = a.view() {
                stack.push(expr);
            }
        }
    }

    /// Marks the functions that the shared expression `e` odr-uses
    /// referenced (which defines or instantiates them on use), warns
    /// again about its null dereferences ([`Sema::reissue_null_derefs`]),
    /// and checks the variadic arguments of its calls that the template
    /// definition recorded ([`Sema::check_variadic_args`]).
    pub(crate) fn mark_shared_expr_referenced(&mut self, e: ExprId) {
        if self.unevaluated > 0 || self.in_template() {
            return;
        }
        let rewarn = self.null_deref_rewarn_active();
        let mut derefs = Vec::new();
        let mut varargs = Vec::new();
        let mut diagnose_ifs = Vec::new();
        // The callees of the immediate invocations (not references).
        let mut callees = Vec::new();
        let mut compares = Vec::new();
        let recompare = !self.null_compare_rewarn.is_empty();
        let mut stack = self.take_walk_stack(e);
        while let Some(x) = stack.pop() {
            let kind = self.ast.exprs.get(x).kind();
            let loc = self.eloc(x);
            if recompare && self.null_compare_rewarn.contains_key(&x) {
                compares.push(x);
            }
            match kind {
                // Unevaluated operands ([expr.context]/1) and the bodies
                // of lambdas (their own functions).
                ExprKind::UnaryExprOrTypeTraitExpr
                | ExprKind::CXXNoexceptExpr
                | ExprKind::ExpressionTraitExpr
                | ExprKind::RequiresExpr
                | ExprKind::ConceptSpecializationExpr
                | ExprKind::LambdaExpr => continue,
                ExprKind::CXXTypeidExpr if !self.ast.exprs.typeid_potentially_evaluated(x) => {
                    continue;
                }
                ExprKind::DeclRefExpr | ExprKind::MemberExpr => {
                    if kind == ExprKind::DeclRefExpr && !callees.contains(&x) {
                        self.recheck_shared_ref(x);
                    }
                    self.reference_shared_name(x, kind, loc, &mut stack);
                }
                ExprKind::CXXConstructExpr
                | ExprKind::CXXTemporaryObjectExpr
                | ExprKind::CXXInheritedCtorInitExpr => {
                    let c = self.ast.exprs.constructor(x);
                    self.mark_function_referenced(c, loc);
                }
                ExprKind::CXXBindTemporaryExpr => {
                    let d = self.ast.exprs.bound_destructor(x);
                    self.mark_function_referenced(d, loc);
                }
                ExprKind::CXXNewExpr => {
                    let n = self.ast.exprs.new_extra(x);
                    let ops = [n.operator_new, n.operator_delete];
                    for f in ops.into_iter().flatten() {
                        self.mark_function_referenced(f, loc);
                    }
                }
                ExprKind::UnaryOperator if rewarn => derefs.push(x),
                // A failed immediate invocation is reported again.
                ExprKind::ConstantExpr => {
                    callees.extend(self.invocation_callee_ref(x));
                    self.recheck_shared_invocation(x);
                }
                ExprKind::CXXDeleteExpr => {
                    if let Some(f) = self.ast.exprs.operator_delete(x) {
                        self.mark_function_referenced(f, loc);
                    }
                }
                _ => {}
            }
            // The children, pushed onto the stack: `stack[base..]`.
            let base = stack.len();
            self.ast.exprs.children_into(x, &self.ast.lists, &mut stack);
            let kids = &stack[base..];
            // The default arguments and member initializers it uses are
            // used again here: their immediate invocations are evaluated
            // (clang's rebuilt `CXXDefaultArgExpr`/`CXXDefaultInitExpr`).
            let at = match kind {
                ExprKind::CallExpr
                | ExprKind::CXXMemberCallExpr
                | ExprKind::CXXOperatorCallExpr => Some(self.begin_loc(self.ast.exprs.callee(x))),
                ExprKind::CXXConstructExpr | ExprKind::CXXTemporaryObjectExpr => Some(loc),
                ExprKind::InitListExpr => self.ast.exprs.end_loc(x),
                _ => None,
            };
            if let Some(at) = at {
                if !self.vararg_recheck.is_empty() && kind != ExprKind::InitListExpr {
                    varargs.extend(
                        kids.iter().filter_map(|k| self.vararg_recheck.get(k).map(|&c| (*k, c))),
                    );
                }
                if !self.diagnose_if_recheck.is_empty() && kind != ExprKind::InitListExpr {
                    diagnose_ifs.extend(
                        kids.iter().copied().filter(|k| self.diagnose_if_recheck.contains_key(k)),
                    );
                }
                self.note_default_uses(kids, at);
            }
        }
        self.put_walk_stack(stack);
        if !derefs.is_empty() {
            self.reissue_null_derefs(&derefs);
        }
        if !compares.is_empty() {
            self.reissue_null_compares(&compares);
        }
        for (a, kind) in varargs {
            self.check_variadic_args(&[a], kind);
        }
        for a in diagnose_ifs {
            self.recheck_diagnose_if(a);
        }
    }

    /// The default arguments and member initializers among `kids` are
    /// used again at `at` ([`Sema::mark_shared_expr_referenced`]).
    fn note_default_uses(&mut self, kids: &[ExprId], at: Loc) {
        for &k in kids {
            match self.ast.exprs.get(k).kind() {
                ExprKind::CXXDefaultArgExpr => {
                    self.note_default_use(k, self.ast.exprs.default_arg_param(k), at);
                }
                ExprKind::CXXDefaultInitExpr => {
                    self.note_default_use(k, self.ast.exprs.default_init_field(k), at);
                }
                _ => {}
            }
        }
    }
}
