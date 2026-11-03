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

use crate::Sema;

impl Sema<'_, '_> {
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
        let mut compares = Vec::new();
        let recompare = !self.null_compare_rewarn.is_empty();
        let mut stack = vec![e];
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
                    if self.ast.exprs.non_odr_use(x) == NonOdrUse::None
                        && self.ast.decls.function(d).is_some()
                    {
                        self.mark_function_referenced(d, loc);
                    }
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
                ExprKind::ConstantExpr => self.recheck_shared_invocation(x),
                ExprKind::CXXDeleteExpr => {
                    if let Some(f) = self.ast.exprs.operator_delete(x) {
                        self.mark_function_referenced(f, loc);
                    }
                }
                _ => {}
            }
            let kids = self.ast.exprs.children(x, &self.ast.lists);
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
                for &k in &kids {
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
            stack.extend(kids);
        }
        if !derefs.is_empty() {
            self.reissue_null_derefs(&derefs);
        }
        if !compares.is_empty() {
            self.reissue_null_compares(&compares);
        }
        for (a, kind) in varargs {
            self.check_variadic_args(&[a], kind);
        }
    }
}
