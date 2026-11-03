//! `-Wint-in-bool-context` and the `-Wtautological-constant-compare`
//! cases of an integer converted to `bool` ([conv.bool]; clang's
//! `DiagnoseIntInBoolContext` and `CheckLogicalOperands`). See
//! spec/sema.md §5.4 "Integers in a boolean context".

use kiln_ast::decl::DeclKind;
use kiln_ast::expr::{BinaryOp, ExprKind, UnaryOp};
use kiln_ast::types::BuiltinType;
use kiln_ast::{ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::conv::rank::{int_signed, int_width};
use crate::impcast::truncate_to;

impl Sema<'_, '_> {
    /// clang's `DiagnoseIntInBoolContext`: `e` (parentheses and implicit
    /// casts stripped) is converted to `bool`. A `<<` whose operands are
    /// both integer literals (or whose left operand is a literal 0) always
    /// gives the same value; another signed `<<` probably meant a
    /// comparison; a conditional whose arms are integer literals, not
    /// both 0 or 1 and both non-zero, is always `true`; its non-`bool`
    /// arms are each converted to `bool` in turn (clang's
    /// `CheckConditionalOperator`).
    pub(crate) fn diagnose_int_in_bool_context(&mut self, e: ExprId) {
        let mut work = vec![e];
        let mut budget = 4096u32;
        while let Some(e) = work.pop() {
            budget = budget.saturating_sub(1);
            if budget == 0 {
                break;
            }
            if let Some(arms) = self.int_in_bool_one(e) {
                for a in arms.into_iter().rev() {
                    let t = self.ast.exprs.get(a).ty();
                    if self.ast.types.builtin(t) != Some(BuiltinType::Bool) {
                        work.push(a);
                    }
                }
            }
        }
    }

    /// One step of [`Self::diagnose_int_in_bool_context`]: the arms of a
    /// conditional `e`.
    fn int_in_bool_one(&mut self, e: ExprId) -> Option<[ExprId; 2]> {
        let e = self.ignore_paren_imp_casts(e);
        let n = *self.ast.exprs.get(e);
        match n.kind() {
            ExprKind::BinaryOperator if self.ast.exprs.binary_op(e) == BinaryOp::Shl => {
                let [l, r] = self.ast.exprs.operands(e);
                let (ll, rl) = (self.bool_ctx_literal(l), self.bool_ctx_literal(r));
                if ll.is_some_and(|x| x.mag == 0) {
                    let d = self.diag(DiagId::sema_left_shift_always, n.loc()).arg(0u32);
                    self.emit(d.per_instantiation());
                } else if let (Some(ll), Some(rl)) = (ll, rl)
                    && !rl.top_bit
                    && !self.expr_is_value_dependent(e)
                    && let Some(v) = self.fold_literal_shl(ll, rl, n.ty())
                {
                    let d = self.diag(DiagId::sema_left_shift_always, n.loc()).arg(u32::from(v));
                    self.emit(d.per_instantiation());
                } else if self.ast.types.is_integral(n.ty()) && int_signed(self, n.ty()) {
                    let d = if self.clang_compat < 22 {
                        let text = self.expr_text_as_written(e);
                        self.diag(DiagId::sema_left_shift_in_bool_context_expr, n.loc()).arg(text)
                    } else {
                        self.diag(DiagId::sema_left_shift_in_bool_context, n.loc())
                    };
                    self.emit(d.per_instantiation());
                }
            }
            ExprKind::ConditionalOperator => {
                let arms = self.ast.exprs.cond_arms(e);
                if let (Some(t), Some(f)) =
                    (self.bool_ctx_literal(arms[0]), self.bool_ctx_literal(arms[1]))
                    && !(t.mag <= 1 && f.mag <= 1)
                    && t.mag != 0
                    && f.mag != 0
                {
                    let at = self.begin_loc(e);
                    let d =
                        self.diag(DiagId::sema_integer_constants_in_conditional_always_true, at);
                    self.emit(d.per_instantiation());
                }
                return Some(arms);
            }
            _ => {}
        }
        None
    }

    /// clang's `getIntegerLiteral`: an `IntegerLiteral`, or one directly
    /// under a unary `-` (whose literal is the one returned); no
    /// parentheses or implicit casts are looked through.
    fn bool_ctx_literal(&self, e: ExprId) -> Option<BoolCtxLit> {
        let n = self.ast.exprs.get(e);
        let (lit, neg) = match n.kind() {
            ExprKind::IntegerLiteral => (e, false),
            ExprKind::UnaryOperator if self.ast.exprs.unary_op(e) == UnaryOp::Minus => {
                let sub = self.ast.exprs.operand(e);
                if self.ast.exprs.get(sub).kind() != ExprKind::IntegerLiteral {
                    return None;
                }
                (sub, true)
            }
            _ => return None,
        };
        let mag = self.ast.exprs.int_value(lit)?;
        let w = int_width(self, self.ast.exprs.get(lit).ty())?.clamp(1, 64);
        Some(BoolCtxLit { mag, neg, top_bit: (mag >> (w - 1)) & 1 == 1 })
    }

    /// `l << r` for literal operands as clang's constant folder computes
    /// it (`handleShiftLeft` outside a constant context): a negative
    /// count shifts right, a count of at least the width shifts by the
    /// width less one, and the result wraps to the type of the shift.
    /// `Some(true)` for a non-zero result.
    fn fold_literal_shl(&self, l: BoolCtxLit, r: BoolCtxLit, ty: QualType) -> Option<bool> {
        let w = int_width(self, ty)?;
        if w == 0 || w > 64 {
            return None;
        }
        let signed = int_signed(self, ty);
        let val = |x: BoolCtxLit| {
            let m = i128::from(x.mag);
            if x.neg { -m } else { m }
        };
        let lv = truncate_to(val(l), w, signed);
        let rv = val(r);
        let max = i128::from(w - 1);
        let res = if rv < 0 { lv >> (-rv).min(max) } else { lv << rv.min(max) };
        Some(truncate_to(res, w, signed) != 0)
    }

    /// clang's `CheckLogicalOperands`: an operand of `&&` / `||` naming
    /// an enumerator whose value is not 0 or 1 (`-Wint-in-bool-context`).
    /// True if one did (`-Wconstant-logical-operand` then stays quiet).
    pub(crate) fn diagnose_enum_constant_in_bool_context(
        &mut self,
        l: ExprId,
        r: ExprId,
        loc: Loc,
    ) -> bool {
        let enum_not_bool = |s: &Self, e: ExprId| {
            s.ast.exprs.get(e).kind() == ExprKind::DeclRefExpr && {
                let d = s.ast.exprs.decl(e);
                s.kind_of(d) == DeclKind::EnumConstant
                    && s.ast
                        .decls
                        .enumerator(d)
                        .and_then(|x| x.value)
                        .and_then(|v| s.const_int(v))
                        .is_some_and(|v| v != 0 && v != 1)
            }
        };
        if !enum_not_bool(self, l) && !enum_not_bool(self, r) {
            return false;
        }
        let d = self.diag(DiagId::sema_enum_constant_in_bool_context, loc);
        self.emit(d.per_instantiation());
        true
    }

    /// In an instantiation, [`Self::diagnose_enum_constant_in_bool_context`]
    /// for each `&&` / `||` of the full-expression `root` (clang rebuilds
    /// every operator and checks it again; a node the instantiation shares
    /// with its pattern was not built again). The operands are taken as
    /// written: implicit conversions stripped, parentheses kept. A
    /// rebuilt operator already reported is deduplicated (spec/diag.md
    /// §7.4).
    pub(crate) fn recheck_enum_constants_in_bool(&mut self, root: ExprId) {
        let mut stack = vec![root];
        let mut budget = 1u32 << 16;
        while let Some(e) = stack.pop() {
            budget = budget.saturating_sub(1);
            if budget == 0 {
                break;
            }
            let n = *self.ast.exprs.get(e);
            match n.kind() {
                ExprKind::UnaryExprOrTypeTraitExpr
                | ExprKind::StmtExpr
                | ExprKind::LambdaExpr
                | ExprKind::CXXDefaultArgExpr
                | ExprKind::CXXDefaultInitExpr => {
                    continue;
                }
                ExprKind::BinaryOperator
                    if matches!(self.ast.exprs.binary_op(e), BinaryOp::LAnd | BinaryOp::LOr) =>
                {
                    let [l, r] = self.ast.exprs.operands(e);
                    let (l, r) = (self.strip_implicit_casts(l), self.strip_implicit_casts(r));
                    self.diagnose_enum_constant_in_bool_context(l, r, n.loc());
                }
                _ => {}
            }
            let kids = self.ast.exprs.children(e, &self.ast.lists);
            stack.extend(kids.into_iter().rev());
        }
    }

    /// `e` without its implicit casts (parentheses kept).
    fn strip_implicit_casts(&self, mut e: ExprId) -> ExprId {
        for _ in 0..256 {
            if self.ast.exprs.get(e).kind() != ExprKind::ImplicitCastExpr {
                break;
            }
            match self.ast.exprs.children(e, &self.ast.lists).first() {
                Some(&c) => e = c,
                None => break,
            }
        }
        e
    }
}

/// An integer literal operand: its magnitude, whether a unary `-`
/// applies, and whether the literal's own top bit is set (clang's
/// `APInt::isNonNegative` on the literal).
#[derive(Clone, Copy)]
struct BoolCtxLit {
    mag: u64,
    neg: bool,
    top_bit: bool,
}
