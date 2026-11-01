//! Marking the declarations of an expression that an instantiation shares
//! with its pattern (spec/sema.md §5.10).
//!
//! A non-dependent expression is not rebuilt by substitution: the
//! instantiation reuses the pattern's node. Inside the template definition
//! its functions were only *referenced* ([temp.res.general]/6), so the
//! instantiation must odr-use them itself, as clang's TreeTransform does by
//! rebuilding the expression (`MarkDeclRefReferenced`, `MarkFunctionReferenced`).
//! The default arguments and default member initializers it uses have
//! their immediate invocations evaluated again. Unevaluated operands and
//! lambda bodies are not entered.

use kiln_ast::ExprId;
use kiln_ast::expr::{ExprKind, NonOdrUse};

use crate::Sema;

impl Sema<'_, '_> {
    /// Marks the functions that the shared expression `e` odr-uses
    /// referenced (which defines or instantiates them on use).
    pub(crate) fn mark_shared_expr_referenced(&mut self, e: ExprId) {
        if self.unevaluated > 0 || self.in_template() {
            return;
        }
        let mut stack = vec![e];
        while let Some(x) = stack.pop() {
            let kind = self.ast.exprs.get(x).kind();
            let loc = self.eloc(x);
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
                ExprKind::CallExpr | ExprKind::CXXMemberCallExpr | ExprKind::CXXOperatorCallExpr => {
                    Some(self.begin_loc(self.ast.exprs.callee(x)))
                }
                ExprKind::CXXConstructExpr | ExprKind::CXXTemporaryObjectExpr => Some(loc),
                ExprKind::InitListExpr => self.ast.exprs.end_loc(x),
                _ => None,
            };
            if let Some(at) = at {
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
    }
}
