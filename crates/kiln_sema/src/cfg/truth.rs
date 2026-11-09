//! The constant truth of branch conditions and the other syntactic
//! facts the CFG builder asks of the AST (spec/sema.md §7.5): clang
//! `CFGBuilder::tryEvaluateBool` with `checkIncorrectLogicOperator`,
//! the labels whose address is taken, and the folded case bounds.

use kiln_ast::decl::DeclKind;
use kiln_ast::expr::{BinaryOp, ExprKind, UnaryOp};
use kiln_ast::{DeclId, ExprId, StmtId};

use crate::Sema;

/// The folded `lo` and `hi` of a case label.
pub(crate) type CaseBounds = (Option<i128>, Option<i128>);

impl Sema<'_, '_> {
    /// The labels of `&&l` expressions in `body`, outside lambdas and
    /// unevaluated operands (the expressions clang's CFG visits).
    pub(crate) fn addr_taken_labels(&self, body: StmtId) -> Vec<DeclId> {
        let (mut stmts, mut exprs, mut out) = (vec![body], Vec::new(), Vec::new());
        while let Some(s) = stmts.pop() {
            self.stmt_parts(s, &mut exprs, &mut stmts);
            while let Some(e) = exprs.pop() {
                match self.ast.exprs.get(e).kind() {
                    ExprKind::LambdaExpr | ExprKind::UnaryExprOrTypeTraitExpr => {}
                    ExprKind::AddrLabelExpr => out.push(self.ast.exprs.addr_label(e)),
                    ExprKind::StmtExpr => stmts.push(self.ast.exprs.stmt_expr_body(e)),
                    _ => self.ast.exprs.children_into(e, &self.ast.lists, &mut exprs),
                }
            }
        }
        out
    }

    pub(crate) fn has_catch_all(&self, s: StmtId) -> bool {
        self.ast
            .lists
            .iter::<StmtId>(self.ast.stmts.try_handlers(s))
            .any(|h| self.ast.stmts.catch_var(h).is_none())
    }

    pub(crate) fn known_truth(&mut self, e: ExprId) -> Option<bool> {
        let n = self.ast.exprs.get(e);
        if n.kind() == ExprKind::BinaryOperator {
            let op = self.ast.exprs.binary_op(e);
            if matches!(op, BinaryOp::LAnd | BinaryOp::LOr) {
                let or = op == BinaryOp::LOr;
                let [l, rr] = self.ast.exprs.operands(e);
                let lv = self.known_truth(l);
                if lv == Some(or) {
                    return lv;
                }
                let rv = self.known_truth(rr);
                return match (lv, rv) {
                    (Some(a), Some(b)) => Some(if or { a || b } else { a && b }),
                    (None, Some(b)) if b == or => Some(b),
                    (None, None) => self.incorrect_logic_op(l, rr, or),
                    _ => None,
                };
            }
        }
        // clang `evaluateAsBooleanConditionNoCache`'s
        // `EvaluateAsBooleanCondition`, which calls `EvaluateAsRValue`:
        // both are time-trace events (spec/driver.md "Time trace").
        let _cond = self.trace_fold("EvaluateAsBooleanCondition", e);
        let _rvalue = self.trace_fold("EvaluateAsRValue", e);
        self.try_fold(e).and_then(|v| self.const_truth(v))
    }

    /// clang `checkIncorrectLogicOperator`: `x OP1 c1 && x OP2 c2` (or
    /// `||`, either side flipped) with integer-literal or same-enumeration
    /// constants is decided when it has one value at the minimum, at
    /// `c1`, just past the smaller constant, at `c2` and at the maximum.
    pub(crate) fn incorrect_logic_op(&mut self, l: ExprId, r: ExprId, or: bool) -> Option<bool> {
        let (x1, op1, c1) = self.normalized_comparison(l)?;
        let (x2, op2, c2) = self.normalized_comparison(r)?;
        if x1 != x2 {
            return None;
        }
        let (e1, e2) = (self.enum_constant(c1), self.enum_constant(c2));
        match (e1, e2) {
            (None, None) => {}
            (Some(a), Some(b)) if self.ast.decls.get(a).ctx() == self.ast.decls.get(b).ctx() => {}
            _ => return None,
        }
        let (t1, t2) = (self.ast.exprs.get(c1).ty(), self.ast.exprs.get(c2).ty());
        let (w, signed) =
            (crate::conv::rank::int_width(self, t1)?, crate::conv::rank::int_signed(self, t1));
        if crate::conv::rank::int_width(self, t2) != Some(w)
            || crate::conv::rank::int_signed(self, t2) != signed
            || w >= 127
        {
            return None;
        }
        let (v1, v2) = (self.fold_int(c1)?, self.fold_int(c2)?);
        let (min, max) = if signed {
            (-(1i128 << (w - 1)), (1i128 << (w - 1)) - 1)
        } else {
            (0, (1i128 << w) - 1)
        };
        let (mut always_true, mut always_false) = (true, true);
        for v in [min, v1, v1.min(v2) + 1, v2, max] {
            let a = compare(op1, v, v1)?;
            let b = compare(op2, v, v2)?;
            let res = if or { a || b } else { a && b };
            always_true &= res;
            always_false &= !res;
        }
        if always_true || always_false { Some(always_true) } else { None }
    }

    /// `x OP c` or `c OP x` (flipped) for a comparison `e` of a variable
    /// with an integer literal or enumerator: `(x's decl, OP, c)`.
    pub(crate) fn normalized_comparison(&self, e: ExprId) -> Option<(DeclId, BinaryOp, ExprId)> {
        let e = self.skip_parens(e);
        if self.ast.exprs.get(e).kind() != ExprKind::BinaryOperator {
            return None;
        }
        let op = self.ast.exprs.binary_op(e);
        if !matches!(
            op,
            BinaryOp::EQ | BinaryOp::NE | BinaryOp::LT | BinaryOp::GT | BinaryOp::LE | BinaryOp::GE
        ) {
            return None;
        }
        let [a, b] = self.ast.exprs.operands(e);
        let (x, op, c) = if let Some(c) = self.int_or_enum_constant(b) {
            (a, op, c)
        } else {
            let flipped = match op {
                BinaryOp::LT => BinaryOp::GT,
                BinaryOp::GT => BinaryOp::LT,
                BinaryOp::LE => BinaryOp::GE,
                BinaryOp::GE => BinaryOp::LE,
                o => o,
            };
            (b, flipped, self.int_or_enum_constant(a)?)
        };
        let mut x = x;
        while matches!(
            self.ast.exprs.get(x).kind(),
            ExprKind::ImplicitCastExpr | ExprKind::ParenExpr
        ) {
            x = self.ast.exprs.operand(x);
        }
        if self.ast.exprs.get(x).kind() != ExprKind::DeclRefExpr {
            return None;
        }
        Some((self.ast.exprs.decl(x), op, c))
    }

    pub(crate) fn skip_parens(&self, mut e: ExprId) -> ExprId {
        while self.ast.exprs.get(e).kind() == ExprKind::ParenExpr {
            e = self.ast.exprs.operand(e);
        }
        e
    }

    /// clang `tryTransformToIntOrEnumConstant`: an integer literal (under
    /// parens, one integral cast and one unary minus) or an enumerator.
    pub(crate) fn int_or_enum_constant(&self, e: ExprId) -> Option<ExprId> {
        let e = self.skip_parens(e);
        let mut x = e;
        let ex = &self.ast.exprs;
        if ex.get(x).kind() == ExprKind::ImplicitCastExpr
            && ex.cast_kind(x) == kiln_ast::expr::CastKind::IntegralCast
        {
            x = ex.operand(x);
        }
        if ex.get(x).kind() == ExprKind::UnaryOperator && ex.unary_op(x) == UnaryOp::Minus {
            x = ex.operand(x);
        }
        if ex.get(x).kind() == ExprKind::IntegerLiteral {
            return Some(e);
        }
        self.enum_constant(e).map(|_| e)
    }

    /// The enumerator `e` names (under parens and implicit casts).
    pub(crate) fn enum_constant(&self, mut e: ExprId) -> Option<DeclId> {
        while matches!(
            self.ast.exprs.get(e).kind(),
            ExprKind::ImplicitCastExpr | ExprKind::ParenExpr
        ) {
            e = self.ast.exprs.operand(e);
        }
        (self.ast.exprs.get(e).kind() == ExprKind::DeclRefExpr)
            .then(|| self.ast.exprs.decl(e))
            .filter(|&d| self.kind_of(d) == DeclKind::EnumConstant)
    }

    /// `e` is `&&` or `||`, parentheses skipped.
    pub(crate) fn is_logical_op(&self, e: ExprId) -> bool {
        let e = self.skip_parens(e);
        self.ast.exprs.get(e).kind() == ExprKind::BinaryOperator
            && matches!(self.ast.exprs.binary_op(e), BinaryOp::LAnd | BinaryOp::LOr)
    }

    /// The folded bounds of case `c`: `lo` and `hi` of a GNU range
    /// `case lo ... hi`, else the value twice.
    pub(crate) fn case_bounds(&mut self, c: StmtId) -> CaseBounds {
        let d = *self.ast.stmts.case_data(c);
        let lo = self.fold_int(d.lhs);
        let hi = match d.rhs {
            Some(h) => self.fold_int(h),
            None => lo,
        };
        (lo, hi)
    }

    /// The case of bounds `b` matches the value `v` (clang CFG
    /// `shouldAddCase`).
    pub(crate) fn in_case(b: CaseBounds, v: i128) -> bool {
        matches!(b, (Some(lo), Some(hi)) if lo <= v && v <= hi)
    }
}

/// clang `analyzeLogicOperatorCondition`: `v OP c`.
fn compare(op: BinaryOp, v: i128, c: i128) -> Option<bool> {
    Some(match op {
        BinaryOp::EQ => v == c,
        BinaryOp::NE => v != c,
        BinaryOp::LT => v < c,
        BinaryOp::GT => v > c,
        BinaryOp::LE => v <= c,
        BinaryOp::GE => v >= c,
        _ => return None,
    })
}
