//! Implicit-conversion warnings over a completed full-expression
//! (clang's `CheckImplicitConversions` / `AnalyzeImplicitConversions`).
//!
//! Only `-Wliteral-conversion` is implemented: a floating literal
//! (possibly negated or parenthesized) implicitly converted to an integer
//! type whose value changes, or is out of range (clang's
//! `DiagnoseFloatingImpCast` with `IsLiteral`).

use kiln_ast::expr::{ExprKind, UnaryOp};
use kiln_ast::types::BuiltinType;
use kiln_ast::{ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::{Loc, Span};

use crate::Sema;
use crate::conv::narrow::float_kind;
use crate::conv::rank::{int_signed, int_width};
use crate::conv::soft;

/// Bound on the nodes looked at in one full-expression.
const MAX_NODES: usize = 1_000_000;

impl Sema<'_, '_> {
    /// Walks full-expression `root`, whose conversion context is `cc`
    /// (the declared variable, `return`, ...; clang's `CC`), diagnosing
    /// its implicit conversions. Children of an expression take its own
    /// location as their context, as in clang.
    pub(crate) fn check_implicit_conversions(&mut self, root: ExprId, cc: Loc) {
        let mut work = vec![(root, cc)];
        let mut seen = 0usize;
        while let Some((e, cc)) = work.pop() {
            seen += 1;
            if seen > MAX_NODES {
                break;
            }
            let s = self.strip_impcast(e);
            if s != e {
                self.check_conversion(e, s, cc);
            }
            let n = *self.ast.exprs.get(s);
            match n.kind() {
                // Not evaluated, or analyzed as statements of their own.
                ExprKind::UnaryExprOrTypeTraitExpr | ExprKind::StmtExpr | ExprKind::LambdaExpr => {
                    continue;
                }
                k if k.is_explicit_cast() => {
                    let sub = self.strip_impcast(self.ast.exprs.operand(s));
                    work.push((sub, cc));
                    continue;
                }
                _ => {}
            }
            let at = match n.kind() {
                ExprKind::CallExpr | ExprKind::CXXMemberCallExpr => self.begin_loc(s),
                _ => n.loc(),
            };
            let kids = self.ast.exprs.children(s, &self.ast.lists);
            work.extend(kids.into_iter().rev().map(|k| (k, at)));
        }
    }

    /// clang's `IgnoreParenImpCasts`.
    fn strip_impcast(&self, mut e: ExprId) -> ExprId {
        for _ in 0..256 {
            match self.ast.exprs.get(e).kind() {
                ExprKind::ParenExpr
                | ExprKind::ImplicitCastExpr
                | ExprKind::ConstantExpr
                | ExprKind::ExprWithCleanups
                | ExprKind::MaterializeTemporaryExpr => e = self.ast.exprs.operand(e),
                ExprKind::SubstNonTypeTemplateParmExpr => e = self.ast.exprs.subst_replacement(e),
                _ => break,
            }
        }
        e
    }

    /// The conversion of `src` (implicit casts and parentheses stripped)
    /// to the type of `e`.
    fn check_conversion(&mut self, e: ExprId, src: ExprId, cc: Loc) {
        let to = self.ast.exprs.get(e).ty();
        let from = self.ast.exprs.get(src).ty();
        if self.ast.types.builtin(to).is_none_or(|b| !b.is_integral()) {
            return;
        }
        if self.ast.types.builtin(from).and_then(|b| float_kind(self, b)).is_none() {
            return;
        }
        self.floating_imp_cast(src, to, cc);
    }

    /// clang's `CheckConditionalOperand` / `DiagnoseFloatingImpCast` for
    /// operand `e` converted to integer type `to`.
    fn floating_imp_cast(&mut self, e: ExprId, to: QualType, cc: Loc) {
        let n = *self.ast.exprs.get(e);
        if n.kind() == ExprKind::ConditionalOperator {
            for arm in self.ast.exprs.cond_arms(e) {
                let a = self.strip_impcast(arm);
                if self
                    .ast
                    .types
                    .builtin(self.ast.exprs.get(a).ty())
                    .and_then(|b| float_kind(self, b))
                    .is_some()
                {
                    self.floating_imp_cast(a, to, cc);
                }
            }
            return;
        }
        let Some((kind, bits)) = self.float_literal_value(e) else { return };
        let is_bool = self.ast.types.builtin(to) == Some(BuiltinType::Bool);
        let (w, signed) = if is_bool {
            (1, false)
        } else {
            (int_width(self, to).unwrap_or(64), int_signed(self, to))
        };
        // A negative zero converts inexactly (`APFloat::convertToInteger`).
        let int = soft::to_int(kind, bits).map(|(neg, mag, exact)| (neg, mag, exact && !(neg && mag == 0)));
        let fits = int.is_some_and(|(neg, mag, _)| representable(neg, mag, w, signed));
        if fits && int.is_some_and(|(_, _, exact)| exact) {
            return;
        }
        let from = self.ty_arg(n.ty());
        let target = self.ty_arg(to.without_quals());
        let at = n.loc();
        let range = Span::new(self.begin_loc(e), self.lit_expr_end(e).unwrap_or(at));
        let ctx = Span::new(cc, self.token_end(cc).unwrap_or(cc));
        let d = if !is_bool && !fits {
            self.diag(DiagId::sema_literal_conversion_out_of_range, at).arg(from).arg(target)
        } else {
            let value = soft::apfloat_diag_string(kind, bits);
            let result = if is_bool {
                if soft::is_zero_value(kind, bits) { "false".to_string() } else { "true".to_string() }
            } else {
                let (neg, mag, _) = int.unwrap_or((false, 0, false));
                if neg && mag != 0 { format!("-{mag}") } else { mag.to_string() }
            };
            self.diag(DiagId::sema_literal_conversion, at).arg(from).arg(target).arg(value).arg(result)
        };
        let d = d.range(range).range(ctx);
        self.emit(d);
    }

    /// The value of a floating literal, possibly under a unary `+`/`-`
    /// (clang's `IsLiteral` case).
    fn float_literal_value(&self, e: ExprId) -> Option<(kiln_ast::consts::FloatKind, u128)> {
        let n = self.ast.exprs.get(e);
        let (lit, negate) = match n.kind() {
            ExprKind::FloatingLiteral => (e, false),
            ExprKind::UnaryOperator => {
                let op = self.ast.exprs.unary_op(e);
                if !matches!(op, UnaryOp::Plus | UnaryOp::Minus) {
                    return None;
                }
                let inner = self.strip_impcast(self.ast.exprs.operand(e));
                if self.ast.exprs.get(inner).kind() != ExprKind::FloatingLiteral {
                    return None;
                }
                (inner, op == UnaryOp::Minus)
            }
            _ => return None,
        };
        let v = self.ast.exprs.literal_const(lit)?;
        let kiln_ast::consts::Scalar::Float { kind, bits, .. } = self.ast.consts.scalar(v)? else {
            return None;
        };
        // The literal's type is the operand's; a cast in between (none in
        // practice) would change the encoding.
        let tk = self.ast.types.builtin(n.ty()).and_then(|b| float_kind(self, b))?;
        if tk != kind {
            return None;
        }
        Some((kind, if negate { bits ^ soft::sign_bit(kind) } else { bits }))
    }

    /// The end of the last token of literal-shaped `e` (a literal under
    /// parentheses and unary operators).
    fn lit_expr_end(&self, e: ExprId) -> Option<Loc> {
        let n = self.ast.exprs.get(e);
        match n.kind() {
            ExprKind::ParenExpr => self.ast.exprs.end_loc(e).map(|r| r.with_offset(1)),
            ExprKind::UnaryOperator | ExprKind::ImplicitCastExpr => {
                self.lit_expr_end(self.ast.exprs.operand(e))
            }
            _ => self.token_end(n.loc()),
        }
    }

    /// The end of the identifier, keyword, number or punctuator token at
    /// `at`, from the source text (a macro location has none).
    pub(crate) fn token_end(&self, at: Loc) -> Option<Loc> {
        let sm = self.diags.sm();
        let (file, pos) = sm.offset_of(at)?;
        let text = sm.text(file).as_bytes();
        let start = pos as usize;
        let mut i = start;
        let word = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'.' || c == b'\'';
        if text.get(i).is_some_and(|&c| word(c)) {
            while let Some(&c) = text.get(i) {
                if word(c) {
                    i += 1;
                } else if (c == b'+' || c == b'-')
                    && i > start
                    && matches!(text[i - 1], b'e' | b'E' | b'p' | b'P')
                    && (text[start].is_ascii_digit() || text[start] == b'.')
                {
                    i += 1;
                } else {
                    break;
                }
            }
        } else {
            i += 1;
        }
        u32::try_from(i).ok().map(|o| sm.loc(file, o))
    }
}

/// True if `neg`/`mag` is a value of a `w`-bit integer of signedness
/// `signed`.
fn representable(neg: bool, mag: u128, w: u32, signed: bool) -> bool {
    if w == 0 {
        return mag == 0;
    }
    if signed {
        let lim = 1u128 << (w.min(128) - 1);
        if neg { mag <= lim } else { mag < lim }
    } else {
        (!neg || mag == 0) && (w >= 128 || mag < (1u128 << w))
    }
}
