//! `-Wnull-dereference` (WP-191): the syntactic `*null` pattern, clang's
//! two `CheckForNullPointerDereference`s (spec/sema.md §5.15).
//!
//! * A load or builtin assignment through `*p` where `p` (parentheses and
//!   casts ignored) is a null pointer constant and the dereference is not
//!   volatile: "indirection of non-volatile null pointer will be deleted,
//!   not trap" with a note suggesting `__builtin_trap()` (clang
//!   `DefaultLvalueConversion`, `CheckAssignmentOperands`).
//! * A reference bound directly to such a `*null`, volatile or not:
//!   "binding dereferenced null pointer to reference has undefined
//!   behavior" (clang `SemaInit`, `SK_BindReference`).
//!
//! Both are run-time behavior diagnostics (clang `DiagRuntimeBehavior`):
//! dropped in unevaluated operands, discarded statements, manifestly
//! constant-evaluated expressions and immediate function contexts
//! (`runtime_diag_quiet`) and when the dereference is unreachable.

use kiln_ast::ExprId;
use kiln_ast::expr::{ExprKind, UnaryOp};
use kiln_ast::types::Quals;
use kiln_diag::DiagId;

use crate::Sema;

impl Sema<'_, '_> {
    /// clang `Expr::IgnoreParenCasts`: parentheses, `__extension__`,
    /// every cast, full-expressions, materialized temporaries and
    /// substituted non-type template parameters.
    pub(crate) fn ignore_paren_casts(&self, e: ExprId) -> ExprId {
        let mut e = e;
        for _ in 0..256 {
            let k = self.ast.exprs.get(e).kind();
            e = match k {
                ExprKind::ParenExpr
                | ExprKind::ConstantExpr
                | ExprKind::ExprWithCleanups
                | ExprKind::MaterializeTemporaryExpr => self.ast.exprs.operand(e),
                ExprKind::UnaryOperator if self.ast.exprs.unary_op(e) == UnaryOp::Extension => {
                    self.ast.exprs.operand(e)
                }
                ExprKind::SubstNonTypeTemplateParmExpr => self.ast.exprs.subst_replacement(e),
                _ if k.is_cast() => self.ast.exprs.operand(e),
                _ => break,
            };
        }
        e
    }

    /// The `*p` under `e` (parentheses and casts ignored) whose operand
    /// is, parentheses and casts ignored, a null pointer constant
    /// (clang `isNullPointerConstant` with `NPC_ValueDependentIsNotNull`:
    /// a zero integer literal, a `std::nullptr_t` value or `__null`).
    fn null_deref(&self, e: ExprId) -> Option<ExprId> {
        let uo = self.ignore_paren_casts(e);
        if self.ast.exprs.get(uo).kind() != ExprKind::UnaryOperator
            || self.ast.exprs.unary_op(uo) != UnaryOp::Deref
        {
            return None;
        }
        let sub = self.ignore_paren_casts(self.ast.exprs.operand(uo));
        let n = self.ast.exprs.get(sub);
        if n.is_value_dependent() {
            return None;
        }
        let null =
            n.kind() == ExprKind::GNUNullExpr || crate::conv::is_null_pointer_constant(self, sub);
        null.then_some(uo)
    }

    /// clang `DiagRuntimeBehavior` anchored at `anchor`. In a template
    /// definition the warning is also kept for the instantiations that
    /// share the expression ([`Sema::reissue_null_deref_warnings`]).
    fn null_deref_diag(&mut self, msg: kiln_diag::Diag, anchor: ExprId) {
        if self.runtime_diag_quiet() {
            return;
        }
        if self.in_template() {
            let n = self.null_deref_rewarn.len();
            self.null_deref_rewarn.entry(anchor).or_insert((n, msg.clone()));
        }
        self.queue_null_deref(msg, anchor);
    }

    /// Queues the null-dereference warning `msg` at the dereference
    /// `uo`. Inside an instantiation it is exempt from the engine's
    /// deduplication against the template definition's report (clang
    /// warns again per instantiation) and deduplicated here instead, per
    /// dereference and instantiation context: a shared dereference that
    /// a rebuilt parent re-checks warns once.
    fn queue_null_deref(&mut self, msg: kiln_diag::Diag, uo: ExprId) {
        let msg = if self.inst.stack.is_empty() {
            msg
        } else {
            if !self.null_deref_seen.insert((uo, self.inst.stack.context_key())) {
                return;
            }
            msg.allow_duplicate()
        };
        self.diag_if_reachable(msg, uo);
    }

    /// Whether an instantiation sharing a non-dependent expression must
    /// look for null dereferences in it to warn about again
    /// ([`Sema::reissue_null_derefs`]).
    pub(crate) fn null_deref_rewarn_active(&self) -> bool {
        !self.null_deref_rewarn.is_empty() && !self.runtime_diag_quiet()
    }

    /// Issues again, at an instantiation sharing them
    /// (spec/sema-templates.md §5.4), the null-dereference warnings that
    /// the template definition found at the dereferences `derefs`: clang rebuilds the expression and
    /// so re-checks it per instantiation. The walk over the shared
    /// expression is [`Sema::mark_shared_expr_referenced`]'s (unevaluated
    /// operands and lambda bodies are not entered), and the warning is
    /// filtered by the instantiation's own reachability. A dereference
    /// whose rebuilt parent re-checks it too warns once
    /// ([`Sema::queue_null_deref`]).
    pub(crate) fn reissue_null_derefs(&mut self, derefs: &[ExprId]) {
        let mut hits: Vec<(usize, ExprId)> = derefs
            .iter()
            .filter_map(|x| self.null_deref_rewarn.get(x).map(|(n, _)| (*n, *x)))
            .collect();
        // In the order the definition checked them.
        hits.sort_unstable();
        for (_, uo) in hits {
            if let Some((_, d)) = self.null_deref_rewarn.get(&uo) {
                let d = d.clone();
                self.queue_null_deref(d, uo);
            }
        }
    }

    /// clang's `CheckForNullPointerDereference` (SemaExpr): `e` is about
    /// to be loaded from (lvalue-to-rvalue converted) or assigned to.
    pub(crate) fn check_null_indirection(&mut self, e: ExprId) {
        let Some(uo) = self.null_deref(e) else { return };
        let sub = self.ast.exprs.operand(uo);
        let sub_ty = self.ast.exprs.get(sub).ty();
        let ty = self.ast.exprs.get(uo).ty();
        if !self.ast.types.is_pointer(sub_ty)
            || self.ast.types.effective_quals(ty).contains(Quals::VOLATILE)
        {
            return;
        }
        let loc = self.ast.exprs.get(uo).loc();
        let note = self.diag(DiagId::note_indirection_through_null, loc);
        let msg = self
            .diag(DiagId::sema_indirection_through_null, loc)
            .range(self.expr_range(sub))
            .with_note(note);
        self.null_deref_diag(msg, uo);
    }

    /// clang's `CheckForNullPointerDereference` (SemaInit): a reference
    /// is bound directly to the glvalue `e`.
    pub(crate) fn check_null_reference_binding(&mut self, e: ExprId) {
        let Some(uo) = self.null_deref(e) else { return };
        let sub = self.ast.exprs.operand(uo);
        let loc = self.ast.exprs.get(uo).loc();
        let msg =
            self.diag(DiagId::sema_binding_null_to_reference, loc).range(self.expr_range(sub));
        self.null_deref_diag(msg, uo);
    }
}
