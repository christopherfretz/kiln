//! `-Winteger-overflow`: a signed `+`, `-`, `*` or unary `-` of a
//! full-expression that is not a constant-expression, whose operands
//! fold and whose result is outside the range of its type, including
//! one inside the body of a `constexpr` function the full-expression
//! calls or a compound assignment there ([expr.pre]/4; clang's
//! `CheckForIntOverflow` and `Expr::EvaluateForOverflow`). See
//! spec/sema.md §5.28.

use kiln_ast::decl::{ConstexprKind, DeclKind};
use kiln_ast::expr::{BinaryOp, CastKind, ExprDeps, ExprKind, UnaryOp};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::conv::rank::{int_signed, int_width};
use crate::impcast::truncate_to;

/// Nodes looked at in one full-expression.
const BUDGET: u32 = 1 << 16;
/// Nesting of evaluated operators followed.
const MAX_DEPTH: u32 = 256;

/// What `overflow_leaf` found.
enum Leaf {
    /// An integer literal's value.
    Int(i128),
    /// A leaf whose value is not known: a read of a variable that is not
    /// `const`, or a literal too wide for an `i128`.
    Unknown,
    /// Not a leaf.
    No,
}

/// The walk's bounds, and what tells its diagnostics apart.
struct Walk {
    budget: u32,
    depth: u32,
    /// The full-expression (for a default argument checked again at a
    /// use, the use: [`Sema::check_default_arg_overflow`]).
    root: ExprId,
    /// The overflows diagnosed so far.
    count: u32,
}

impl Walk {
    /// Takes one node from the budget; false when it is spent.
    fn take(&mut self) -> bool {
        if self.budget == 0 {
            return false;
        }
        self.budget -= 1;
        true
    }
}

impl Sema<'_, '_> {
    /// clang's `CheckForIntOverflow`, from `CheckCompletedExpr`: the
    /// full-expression `root` of a context that is not constant-evaluated
    /// (not a constant-expression, not a `constexpr` variable's
    /// initializer, whose evaluation reports the overflow as an error).
    /// A binary or unary operator (parentheses and casts ignored) is
    /// evaluated; the elements of an initializer list, the arguments of
    /// a call or a construction, an array index, a compound literal's
    /// initializer and an array `new`'s size are looked at in turn.
    pub(crate) fn check_for_int_overflow(&mut self, root: ExprId, var_init: Option<DeclId>) {
        self.check_for_int_overflow_as(root, root, var_init);
    }

    /// The default argument `root` used by `use_expr`, checked again
    /// there: clang rebuilds a default argument with an immediate
    /// invocation or a source-location builtin at each use and completes
    /// it again (`BuildCXXDefaultArgExpr` → `ConvertParamDefaultArgument`
    /// → `CheckCompletedExpr`), so its overflows warn at each such use
    /// too, before the use's invocations are checked (WP-1171).
    pub(crate) fn check_default_arg_overflow(&mut self, use_expr: ExprId, root: ExprId) {
        self.check_for_int_overflow_as(root, use_expr, None);
    }

    /// [`Sema::check_for_int_overflow`] of `root`, its warnings told
    /// apart from another check's by `key` (the full-expression checked).
    fn check_for_int_overflow_as(&mut self, root: ExprId, key: ExprId, var_init: Option<DeclId>) {
        if self.constant_full_expr > 0
            || self.unevaluated > 0
            || self.in_constant_evaluated()
            || !self.diags.is_enabled_at(
                DiagId::sema_integer_constant_overflow,
                self.ast.exprs.get(root).loc(),
            )
            || self
                .ast
                .exprs
                .get(root)
                .deps()
                .intersects(ExprDeps::TYPE.with(ExprDeps::VALUE).with(ExprDeps::ERROR))
            || var_init
                .is_some_and(|v| self.ast.decls.var(v).is_some_and(|d| d.spec.is_constexpr()))
            || !self.has_overflow_candidate(root)
        {
            return;
        }
        let mut w = Walk { budget: BUDGET, depth: 0, root: key, count: 0 };
        let mut work = vec![root];
        while let Some(orig) = work.pop() {
            if !w.take() {
                return;
            }
            let e = self.ignore_paren_casts(orig);
            let k = self.ast.exprs.get(e).kind();
            if matches!(
                k,
                ExprKind::BinaryOperator
                    | ExprKind::CompoundAssignOperator
                    | ExprKind::UnaryOperator
            ) {
                let _ = self.overflow_value(e, &mut w, false);
                continue;
            }
            // An initializer list as written, not under a cast
            // (`int{x}`); a materialized temporary is looked through.
            let mut o = orig;
            while self.ast.exprs.get(o).kind() == ExprKind::MaterializeTemporaryExpr {
                o = self.ast.exprs.operand(o);
            }
            if self.ast.exprs.get(o).kind() == ExprKind::InitListExpr {
                let mut c = self.ast.exprs.children(o, &self.ast.lists);
                c.reverse();
                work.extend(c);
                continue;
            }
            match k {
                ExprKind::CallExpr
                | ExprKind::CXXMemberCallExpr
                | ExprKind::CXXOperatorCallExpr
                | ExprKind::CXXConstructExpr
                | ExprKind::CXXTemporaryObjectExpr
                | ExprKind::CXXBindTemporaryExpr
                | ExprKind::CompoundLiteralExpr => {
                    let mut c = self.ast.exprs.children(e, &self.ast.lists);
                    c.reverse();
                    work.extend(c);
                }
                ExprKind::ArraySubscriptExpr => work.push(self.ast.exprs.operands(e)[1]),
                ExprKind::CXXNewExpr => work.extend(self.ast.exprs.new_extra(e).array_size),
                _ => {}
            }
        }
    }

    /// Whether `e` contains a signed `+`, `-`, `*` or unary `-`, or a
    /// call to a `constexpr` function (a cheap filter before anything is
    /// folded).
    fn has_overflow_candidate(&self, e: ExprId) -> bool {
        let mut stack = vec![e];
        let mut budget = BUDGET;
        while let Some(x) = stack.pop() {
            if budget == 0 {
                return false;
            }
            budget -= 1;
            if self.overflow_candidate(x).is_some() || self.overflow_call(x) {
                return true;
            }
            if !self.overflow_opaque(x) {
                self.ast.exprs.children_into(x, &self.ast.lists, &mut stack);
            }
        }
        false
    }

    /// The type of `e` if it is an operator that can overflow: a signed
    /// integer `+`, `-`, `*` or unary `-` of at most 128 bits.
    fn overflow_candidate(&self, e: ExprId) -> Option<(QualType, u32)> {
        let n = *self.ast.exprs.get(e);
        let arith = match n.kind() {
            ExprKind::BinaryOperator => {
                matches!(self.ast.exprs.binary_op(e), BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul)
            }
            ExprKind::UnaryOperator => self.ast.exprs.unary_op(e) == UnaryOp::Minus,
            _ => false,
        };
        if !arith || !self.ast.types.is_integral(n.ty()) || !int_signed(self, n.ty()) {
            return None;
        }
        let w = int_width(self, n.ty())?;
        (w <= 128).then_some((n.ty(), w))
    }

    /// Whether `e` calls a `constexpr` or `consteval` function or
    /// constructor, whose body the evaluation enters.
    fn overflow_call(&self, e: ExprId) -> bool {
        let f = match self.ast.exprs.get(e).kind() {
            ExprKind::CallExpr | ExprKind::CXXMemberCallExpr | ExprKind::CXXOperatorCallExpr => {
                self.callee_decl(e)
            }
            ExprKind::CXXConstructExpr | ExprKind::CXXTemporaryObjectExpr => {
                Some(self.ast.exprs.constructor(e))
            }
            _ => None,
        };
        f.and_then(|f| self.ast.decls.function(f))
            .is_some_and(|f| f.spec.constexpr_kind() != ConstexprKind::None)
    }

    /// Whether `e` or an expression under it that the evaluation enters
    /// is an [`Sema::overflow_call`].
    fn has_overflow_call(&self, e: ExprId) -> bool {
        let mut stack = vec![e];
        let mut budget = BUDGET;
        while let Some(x) = stack.pop() {
            if budget == 0 {
                return false;
            }
            budget -= 1;
            if self.overflow_call(x) {
                return true;
            }
            if !self.overflow_opaque(x) {
                self.ast.exprs.children_into(x, &self.ast.lists, &mut stack);
            }
        }
        false
    }

    /// Whether a fold of `e` certainly fails, so it is not tried
    /// (WP-798): an operand every evaluation of `e` reaches calls a
    /// member function the Oven cannot call ([expr.const]/10.2), by the
    /// Oven's own rule (`compile_func`: not `constexpr` on its latest
    /// declaration). Virtual, trivial and deleted functions and a
    /// closure's conversion function or invoker, which the Oven handles
    /// otherwise, do not count. The fold would compile a thunk and fail
    /// at that call; skipping it saves the compile
    /// (bench/overload/arrow-chain.cpp: 12,000 operands, each a chain
    /// of 50 non-`constexpr` `operator->` calls). Only operands no
    /// condition guards are looked at: the walk stops at anything but a
    /// call, a cast, a member access, a temporary, a unary operator or
    /// a binary one other than `&&` and `||`.
    fn overflow_fold_fails(&self, e: ExprId) -> bool {
        let mut stack = vec![e];
        let mut budget = BUDGET;
        while let Some(x) = stack.pop() {
            if budget == 0 {
                return false;
            }
            budget -= 1;
            match self.ast.exprs.get(x).kind() {
                ExprKind::CallExpr
                | ExprKind::CXXMemberCallExpr
                | ExprKind::CXXOperatorCallExpr => {
                    if self.callee_decl(x).is_some_and(|d| self.oven_cannot_call(d)) {
                        return true;
                    }
                }
                ExprKind::BinaryOperator
                    if matches!(self.ast.exprs.binary_op(x), BinaryOp::LAnd | BinaryOp::LOr) =>
                {
                    continue;
                }
                ExprKind::ParenExpr
                | ExprKind::ImplicitCastExpr
                | ExprKind::MemberExpr
                | ExprKind::MaterializeTemporaryExpr
                | ExprKind::CXXBindTemporaryExpr
                | ExprKind::ExprWithCleanups
                | ExprKind::UnaryOperator
                | ExprKind::BinaryOperator => {}
                _ => continue,
            }
            self.ast.exprs.children_into(x, &self.ast.lists, &mut stack);
        }
        false
    }

    /// Whether the Oven traps on a call of member function `d` (see
    /// [`Sema::overflow_fold_fails`]).
    fn oven_cannot_call(&self, d: DeclId) -> bool {
        let latest = self.ast.latest(d);
        self.ast.decls.get(d).kind() == DeclKind::CXXMethod
            && self.ast.closure_thunk(latest).is_none()
            && self.ast.decls.builtin_id(d).is_none()
            && self.ast.decls.gnu_lib_twin(d).is_none()
            && self.meta_intrinsic_of(d).is_none()
            && self
                .ast
                .decls
                .function(d)
                .is_some_and(|f| !f.spec.is_virtual() && !f.spec.is_trivial())
            && self.ast.decls.function(latest).is_some_and(|f| {
                f.spec.constexpr_kind() == ConstexprKind::None && !f.spec.is_deleted()
            })
    }

    /// Reports `-Winteger-overflow` at `loc` for operation `e` of type
    /// `ty`, whose result wrapped is `wrapped`. clang reports it straight
    /// to the diagnostics engine: no instantiation notes, and every
    /// overflow found prints, also one at the same place as another (a
    /// function called twice); the same full-expression checked again
    /// does not repeat them.
    fn emit_overflow(&mut self, w: &mut Walk, loc: Loc, e: ExprId, wrapped: i128, ty: QualType) {
        let mut key = self.inst.stack.context_key();
        key ^= (u64::from(w.root.index()) << 20 | u64::from(w.count))
            .wrapping_mul(0x9E37_79B9_7F4A_7C15);
        w.count += 1;
        let d = self
            .diag(DiagId::sema_integer_constant_overflow, loc)
            .arg(group_digits(wrapped))
            .arg(self.ty_arg(ty))
            .range(self.expr_range(e))
            .context(key)
            .per_instantiation();
        d.emit(self.diags);
    }

    /// clang `EvaluateForOverflow` of `e` in the evaluator: the
    /// overflows found on the way are reported, in order, but those of a
    /// `++` or `--` (only a note in clang). `Err` when the evaluation
    /// failed and found none.
    fn overflow_eval(&mut self, e: ExprId, w: &mut Walk) -> Result<Option<i128>, ()> {
        let (v, found) = self.fold_for_overflow(e);
        if v.is_none() && found.is_empty() {
            return Err(());
        }
        for o in found {
            let Some(oe) = o.e else { continue };
            let n = *self.ast.exprs.get(oe);
            if n.kind() == ExprKind::UnaryOperator
                && matches!(
                    self.ast.exprs.unary_op(oe),
                    UnaryOp::PreInc | UnaryOp::PostInc | UnaryOp::PreDec | UnaryOp::PostDec
                )
            {
                continue;
            }
            let ty = n.ty();
            if !self.ast.types.is_integral(ty) || !int_signed(self, ty) {
                continue;
            }
            self.emit_overflow(w, o.loc, oe, o.value, ty);
        }
        Ok(v.and_then(|v| self.const_int(v)))
    }

    /// Subexpressions the evaluation does not enter: unevaluated
    /// operands, and bodies checked as statements of their own.
    fn overflow_opaque(&self, e: ExprId) -> bool {
        matches!(
            self.ast.exprs.get(e).kind(),
            ExprKind::UnaryExprOrTypeTraitExpr
                | ExprKind::LambdaExpr
                | ExprKind::StmtExpr
                | ExprKind::CXXTypeidExpr
                | ExprKind::CXXNoexceptExpr
                | ExprKind::RequiresExpr
        )
    }

    /// clang's `EvaluateForOverflow` of `e`: its integer value if it
    /// folds, where an overflowing operator, once diagnosed, yields its
    /// value wrapped to its type and the evaluation goes on. Without
    /// `need`, only its overflows are wanted: an operand without
    /// operators is not folded.
    fn overflow_value(&mut self, e: ExprId, w: &mut Walk, need: bool) -> Option<i128> {
        if !w.take() || w.depth >= MAX_DEPTH {
            return None;
        }
        w.depth += 1;
        let v = self.overflow_value_inner(e, w, need);
        w.depth -= 1;
        v
    }

    fn overflow_value_inner(&mut self, e: ExprId, w: &mut Walk, need: bool) -> Option<i128> {
        let n = *self.ast.exprs.get(e);
        if n.deps().intersects(ExprDeps::TYPE.with(ExprDeps::VALUE).with(ExprDeps::ERROR)) {
            return None;
        }
        if let Some((ty, width)) = self.overflow_candidate(e) {
            // The exact value, if it is an `i128`, and the value wrapped.
            let (exact, wrapped) = if n.kind() == ExprKind::UnaryOperator {
                let v = self.overflow_value(self.ast.exprs.operand(e), w, true)?;
                (v.checked_neg(), v.wrapping_neg())
            } else {
                let op = self.ast.exprs.binary_op(e);
                let [l, r] = self.ast.exprs.operands(e);
                // Both sides are evaluated, whether or not the left one
                // folds (clang keeps evaluating to find the overflows).
                let l = self.overflow_value(l, w, true);
                let r = self.overflow_value(r, w, l.is_some());
                let (l, r) = (l?, r?);
                match op {
                    BinaryOp::Add => (l.checked_add(r), l.wrapping_add(r)),
                    BinaryOp::Sub => (l.checked_sub(r), l.wrapping_sub(r)),
                    _ => (l.checked_mul(r), l.wrapping_mul(r)),
                }
            };
            let (min, max) = if width == 128 {
                (i128::MIN, i128::MAX)
            } else {
                (-(1i128 << (width - 1)), (1i128 << (width - 1)) - 1)
            };
            if let Some(x) = exact
                && (min..=max).contains(&x)
            {
                return Some(x);
            }
            let wrapped = truncate_to(wrapped, width, true);
            // clang shows the wrapped result; for a unary `-` that is its
            // operand, the minimum value, again.
            self.emit_overflow(w, n.loc(), e, wrapped, ty);
            return Some(wrapped);
        }
        match n.kind() {
            ExprKind::ParenExpr => return self.overflow_value(self.ast.exprs.operand(e), w, need),
            k if k.is_cast()
                && matches!(
                    self.ast.exprs.cast_kind(e),
                    CastKind::IntegralCast | CastKind::NoOp
                )
                && self.ast.types.is_integral(n.ty()) =>
            {
                let v = self.overflow_value(self.ast.exprs.operand(e), w, need)?;
                let width = int_width(self, n.ty())?;
                return (width <= 128).then(|| truncate_to(v, width, int_signed(self, n.ty())));
            }
            ExprKind::ConditionalOperator => {
                let c = self.ast.exprs.children(e, &self.ast.lists);
                let arms = self.ast.exprs.cond_arms(e);
                if let Some(t) = self.fold_rvalue(c[0]).and_then(|v| self.const_truth(v)) {
                    return self.overflow_value(if t { arms[0] } else { arms[1] }, w, need);
                }
                let _ = self.overflow_value(c[0], w, false);
                let _ = self.overflow_value(arms[0], w, false);
                let _ = self.overflow_value(arms[1], w, false);
                return None;
            }
            _ => {}
        }
        match self.overflow_leaf(e) {
            Leaf::Int(v) => return Some(v),
            Leaf::Unknown => return None,
            Leaf::No => {}
        }
        // Anything that calls a `constexpr` function is evaluated, the
        // body included. Anything else, or a failed evaluation that found
        // nothing: its operators are looked for, and it is folded when
        // there are none (a fold fails on an overflow inside).
        let calls = !self.overflow_opaque(e) && self.has_overflow_call(e);
        if calls && let Ok(v) = self.overflow_eval(e, w) {
            return v;
        }
        if (self.overflow_opaque(e) || !self.overflow_scan(e, w))
            && need
            && !calls
            && !self.overflow_fold_fails(e)
        {
            return self.fold_rvalue_int(e);
        }
        None
    }

    /// The value of a leaf that needs no evaluation: an integer literal,
    /// or (`Leaf::Unknown`) a read of a variable that is not `const`.
    fn overflow_leaf(&self, e: ExprId) -> Leaf {
        let n = *self.ast.exprs.get(e);
        match n.kind() {
            ExprKind::IntegerLiteral => match self.ast.exprs.int_value(e) {
                Some(v) => Leaf::Int(i128::from(v)),
                None => Leaf::Unknown,
            },
            ExprKind::ImplicitCastExpr
                if self.ast.exprs.cast_kind(e) == CastKind::LValueToRValue =>
            {
                let r = self.ignore_parens(self.ast.exprs.operand(e));
                if self.ast.exprs.get(r).kind() != ExprKind::DeclRefExpr {
                    return Leaf::No;
                }
                let d = self.ast.exprs.decl(r);
                let dn = self.ast.decls.get(d);
                let Some(ty) = dn.ty() else { return Leaf::No };
                let plain = matches!(dn.kind(), DeclKind::Var | DeclKind::ParmVar)
                    && !self.ast.types.is_reference(ty)
                    && !self.ast.types.effective_quals(ty).is_const();
                if plain { Leaf::Unknown } else { Leaf::No }
            }
            _ => Leaf::No,
        }
    }

    /// Looks for the operators under `e` (`e` is not one), evaluating
    /// each one found; whether there was one. A conditional's arm or a
    /// logical operator's right operand not evaluated is skipped.
    fn overflow_scan(&mut self, e: ExprId, w: &mut Walk) -> bool {
        let mut found = false;
        let mut stack = vec![e];
        while let Some(x) = stack.pop() {
            if !w.take() {
                return true;
            }
            if self.overflow_opaque(x) {
                continue;
            }
            let k = self.ast.exprs.get(x).kind();
            // A valued immediate invocation is a leaf (clang's
            // `VisitConstantExpr` reads the value): one of a default
            // argument too, valued at the use (WP-1171).
            if k == ExprKind::ConstantExpr
                && (self.ast.exprs.constant_value(x).is_some()
                    || self.ast.exprs.default_immediate_value(x).is_some())
            {
                continue;
            }
            // A default argument is evaluated with the call, whatever the
            // callee (clang's `handleCallExpr` evaluates the arguments
            // before it checks the callee is `constexpr`): its overflows
            // are found again (WP-1171).
            if k == ExprKind::CXXDefaultArgExpr {
                stack.extend(self.ast.default_arg_expr(x));
                continue;
            }
            if x != e && self.overflow_call(x) {
                found = true;
                let _ = self.overflow_value(x, w, false);
                continue;
            }
            if x != e
                && (self.overflow_candidate(x).is_some() || k == ExprKind::ConditionalOperator)
            {
                if self.has_overflow_candidate(x) {
                    found = true;
                    let _ = self.overflow_value(x, w, false);
                }
                continue;
            }
            if k == ExprKind::BinaryOperator
                && matches!(self.ast.exprs.binary_op(x), BinaryOp::LAnd | BinaryOp::LOr)
            {
                let [l, r] = self.ast.exprs.operands(x);
                let or = self.ast.exprs.binary_op(x) == BinaryOp::LOr;
                stack.push(l);
                if !self.has_overflow_candidate(r) {
                    continue;
                }
                // The right operand is evaluated unless the left one
                // decides the result.
                if self.fold_rvalue(l).and_then(|v| self.const_truth(v)) != Some(or) {
                    stack.push(r);
                }
                continue;
            }
            let mut c = self.ast.exprs.children(x, &self.ast.lists);
            c.reverse();
            stack.extend(c);
        }
        found
    }
}

/// `v` in decimal with `'` between groups of three digits (clang's
/// `toString` with `InsertSeparators`): `-2'147'483'648`.
fn group_digits(v: i128) -> String {
    let digits = v.unsigned_abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    if v < 0 {
        out.push('-');
    }
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push('\'');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::group_digits;

    #[test]
    fn groups() {
        assert_eq!(group_digits(0), "0");
        assert_eq!(group_digits(-2), "-2");
        assert_eq!(group_digits(999), "999");
        assert_eq!(group_digits(1000), "1'000");
        assert_eq!(group_digits(-2_147_483_648), "-2'147'483'648");
        assert_eq!(group_digits(i128::from(i64::MIN)), "-9'223'372'036'854'775'808");
    }
}
