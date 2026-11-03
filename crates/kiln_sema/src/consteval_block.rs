//! consteval-block-declarations ([dcl.pre], P3289R1;
//! spec/consteval-blocks.md §2): the call of the block's synthesized
//! `consteval` lambda (built by the parser in a manifestly
//! constant-evaluated context) is evaluated once, where the declaration
//! is, as the one plainly constant-evaluated context
//! ([`EvalContext::consteval_block`]). Never codegen'd.

use kiln_ast::decl::DeclFlags;
use kiln_ast::expr::BinaryOp;
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_oven::EvalContext;
use kiln_source::Loc;

use crate::Sema;

impl Sema<'_, '_> {
    /// `consteval { ... }` at `loc`; `call` is the call of its lambda
    /// (`None` after a parse error).
    pub fn act_on_consteval_block(&mut self, call: Option<ExprId>, loc: Loc) -> DeclId {
        let call = call.unwrap_or_else(|| self.recover_err(&[], loc));
        self.consteval_block_checked(call, loc).0
    }

    /// [`Sema::act_on_consteval_block`] for a built call (also an
    /// instantiation's), with whether the block failed. In a template
    /// definition the block waits for instantiation ([temp.inst]: it is
    /// evaluated once per instantiation of the enclosing entity).
    pub(crate) fn consteval_block_checked(&mut self, call: ExprId, loc: Loc) -> (DeclId, bool) {
        let c = self.act_on_constant_full_expr(call);
        let in_definition =
            self.in_template() || self.ast.decls.get(self.ctx).has(DeclFlags::DEPENDENT_CTX);
        let mut failed = self.has_errors_in(c);
        if !failed && !in_definition && !self.expr_is_value_dependent(c) {
            failed = !self.evaluate_consteval_block(c, loc);
        }
        let ctx = self.ctx;
        (self.builder().consteval_block(ctx, loc, c), failed)
    }

    /// Evaluates the block's call `c`: a failure is an error with the
    /// evaluation's notes (a hard error, never a substitution failure).
    fn evaluate_consteval_block(&mut self, c: ExprId, loc: Loc) -> bool {
        // The Oven freezes a value, so evaluate `c, true` (the
        // `static_assert((void)c, true)` equivalence, [dcl.pre]/P3289R1).
        let t = self.builder().bool_literal(QualType::BOOL, true, loc);
        let e = self.builder().binary(BinaryOp::Comma, c, t, QualType::BOOL, loc);
        match self.eval_const(e, EvalContext::consteval_block(loc)) {
            Ok(_) => true,
            // A variable whose initializer failed was diagnosed already.
            Err(_) if self.refers_to_invalid_var(c) => false,
            Err(mut f) => {
                let at = Self::fold_lone_invalid(&mut f, loc);
                let d = self.diag(DiagId::sema_consteval_block_non_constant, at);
                let d = Self::with_failure_notes(d, f);
                self.emit(d);
                false
            }
        }
    }
}
