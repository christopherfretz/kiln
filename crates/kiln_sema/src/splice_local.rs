//! Local entities named by a reflection or a splice across a lambda
//! (spec/reflection.md §5; [expr.reflect]/7.1, [expr.prim.splice]/2.1.4,
//! [basic.def.odr]/5).
//!
//! `^^x` of a local entity `x` is ill-formed when a lambda scope
//! intervenes between `x`'s declaration and the reflect-expression, or
//! when `x` is an init-capture; a splice-expression designating such a
//! local is ill-formed when it is potentially evaluated, disregarding
//! any enclosing `typeid`. Neither is an odr-use, so neither is ever a
//! capture: a capture-default does not make them valid. A splice
//! operand is itself a converted constant expression, so a variable
//! usable in constant expressions that it names (`constexpr auto r =
//! ^^g; [] { return [: r :]; }`) is not odr-used either.

use kiln_ast::expr::{CastKind, ExprKind, NonOdrUse};
use kiln_ast::{DeclId, ExprId};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;

impl Sema<'_, '_> {
    /// Enters the expression operand of `typeid` ([expr.typeid]): an
    /// unevaluated operand that [expr.prim.splice]/2.1.4 nevertheless
    /// treats as potentially evaluated.
    pub fn enter_typeid_operand(&mut self) {
        self.enter_unevaluated();
        self.typeid_operands.push((self.lambdas.len(), self.unevaluated));
    }

    /// Leaves a `typeid` operand entered by
    /// [`Sema::enter_typeid_operand`].
    pub fn exit_typeid_operand(&mut self) {
        self.typeid_operands.pop();
        self.exit_unevaluated();
    }

    /// Whether an expression here would be potentially evaluated if the
    /// effect of every enclosing `typeid` were disregarded: each
    /// unevaluated level of the current lambda body is a `typeid`
    /// operand.
    fn evaluated_but_for_typeid(&self) -> bool {
        let mut u = self.unevaluated;
        let depth = self.lambdas.len();
        for &(l, at) in self.typeid_operands.iter().rev() {
            if u == 0 || l != depth || at != u {
                break;
            }
            u -= 1;
        }
        u == 0
    }

    /// Whether a lambda scope intervenes between local entity `d`'s
    /// declaration and the current context ([basic.pre]: `d` has
    /// automatic storage; the current context is nested in a lambda's
    /// call operator that is itself nested in `d`'s home).
    pub(crate) fn lambda_intervenes(&self, d: DeclId) -> bool {
        if !self.has_local_storage(d) {
            return false;
        }
        let Some(home) = self.ast.decls.get(d).ctx() else { return false };
        // The lambda scope starts after the lambda-introducer
        // ([basic.scope.lambda]): in a declarator still being parsed, the
        // context is the enclosing one, and only the lambda's own
        // parameters, not yet adopted by its call operator, are declared
        // inside it.
        if let Some(l) = self.lambdas.get(self.lambda_base..).and_then(<[_]>::last)
            && !l.body_open
            && l.saved_ctx == self.ctx
            && !self.unowned_params.contains(&d)
        {
            return true;
        }
        let mut c = Some(self.ctx);
        for _ in 0..4096 {
            let Some(x) = c else { return false };
            if x == home {
                return false;
            }
            if self.lambda_of_call_op(x).is_some() {
                return true;
            }
            c = self.ast.decls.lexical_ctx(x).or_else(|| self.ast.decls.get(x).ctx());
        }
        false
    }

    /// [expr.reflect]/7.1: `^^x` for an unqualified `x` naming `d` at
    /// `name_loc` is ill-formed if `d` is declared by an init-capture
    /// (/7.1.1) or is a local entity across an intervening lambda
    /// (/7.1.4). Diagnosed (true) at the name, as GCC 16.1.
    pub(crate) fn reflect_local_across_lambda(&mut self, d: DeclId, name_loc: Loc) -> bool {
        let id = if self.init_capture_owner(d).is_some() {
            DiagId::sema_reflect_init_capture
        } else if self.lambda_intervenes(d) {
            DiagId::sema_reflect_intervening_lambda
        } else {
            return false;
        };
        self.error(id, name_loc);
        true
    }

    /// [expr.prim.splice]/2.1.4: a potentially-evaluated (disregarding
    /// `typeid`) splice-expression at `at` designating local entity `d`
    /// across an intervening lambda. Diagnosed (true).
    pub(crate) fn splice_local_across_lambda(&mut self, d: DeclId, at: Loc) -> bool {
        if !self.evaluated_but_for_typeid() || !self.lambda_intervenes(d) {
            return false;
        }
        let msg = self.diag(DiagId::sema_splice_intervening_lambda, at).arg(self.decl_name(d));
        let note = self
            .diag(DiagId::note_entity_declared_at, self.ast.decls.get(d).loc())
            .arg(format!("'{}'", self.decl_name(d)));
        self.emit(msg.with_note(note));
        true
    }

    /// [basic.def.odr]/5: splice operand `e` is a converted constant
    /// expression, so each potential result of it naming a variable
    /// usable in constant expressions is not odr-used. The operand is not
    /// kept in the expression the full-expression walk
    /// ([`Sema::act_on_full_expr_at`]) visits, so it is resolved here.
    pub(crate) fn splice_operand_not_odr_used(&mut self, e: ExprId) {
        if self.pending_odr.is_empty() && self.potential_caps.is_empty() {
            return;
        }
        let mut e = e;
        for _ in 0..64 {
            match self.ast.exprs.get(e).kind() {
                ExprKind::ConstantExpr => e = self.ast.exprs.operand(e),
                ExprKind::ImplicitCastExpr
                    if matches!(
                        self.ast.exprs.cast_kind(e),
                        CastKind::LValueToRValue | CastKind::NoOp
                    ) =>
                {
                    e = self.ast.exprs.operand(e);
                }
                _ => break,
            }
        }
        let mut found = Vec::new();
        self.potential_results(e, &mut found);
        for r in found {
            let d = self.ast.exprs.decl(r);
            if !self.usable_in_constant_expressions(d) {
                continue;
            }
            if let Some(i) = self.pending_odr.iter().position(|&x| x == r) {
                self.pending_odr.remove(i);
                let _ = self.ast.exprs.set_non_odr_use(r, NonOdrUse::Constant);
            }
            self.potential_caps.retain(|&x| x != r);
        }
    }
}
