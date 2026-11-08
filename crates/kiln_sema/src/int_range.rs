//! clang's `IntRange` / `GetExprRange` (SemaChecking.cpp) in its
//! approximate form: the likely number of significant bits of an integer
//! expression, assuming arithmetic on narrower types stays in them. Used
//! by `-Wimplicit-int-float-conversion` (`impcast.rs`).

use kiln_ast::ExprId;
use kiln_ast::QualType;
use kiln_ast::expr::{BinaryOp, CastKind, ExprKind, UnaryOp};
use kiln_ast::types::{BuiltinType, TypeView};

use crate::Sema;
use crate::conv::rank::{enum_data, int_signed, int_width};

/// Bound on the recursion through one expression.
const MAX_DEPTH: u32 = 64;

/// clang's `IntRange`: `width` active bits, including exactly one sign bit
/// unless `non_negative`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct IntRange {
    pub width: u32,
    pub non_negative: bool,
}

impl IntRange {
    const BOOL: IntRange = IntRange { width: 1, non_negative: true };

    fn value_bits(self) -> u32 {
        if self.non_negative { self.width } else { self.width.saturating_sub(1) }
    }

    /// The conservative merge of two ranges.
    fn join(l: IntRange, r: IntRange) -> IntRange {
        let unsigned = l.non_negative && r.non_negative;
        IntRange {
            width: l.value_bits().max(r.value_bits()) + u32::from(!unsigned),
            non_negative: unsigned,
        }
    }

    /// The range of a bitwise and.
    fn bit_and(l: IntRange, r: IntRange) -> IntRange {
        let mut bits = l.width.max(r.width);
        let mut non_negative = false;
        for x in [l, r] {
            if x.non_negative {
                bits = bits.min(x.width);
                non_negative = true;
            }
        }
        IntRange { width: bits, non_negative }
    }

    /// The range of a remainder: no wider than either side, signed as the
    /// left.
    fn rem(l: IntRange, r: IntRange) -> IntRange {
        let unsigned = l.non_negative;
        IntRange {
            width: l.value_bits().min(r.value_bits()) + u32::from(!unsigned),
            non_negative: unsigned,
        }
    }

    /// clang's `GetValueRange` for the constant `v` of a type that is
    /// `signed` or not, truncated to `max_width` bits.
    fn of_value(v: i128, signed: bool, max_width: u32) -> IntRange {
        if signed && v < 0 {
            return IntRange { width: 129 - (!v).leading_zeros(), non_negative: false };
        }
        #[allow(clippy::cast_sign_loss, reason = "the two's complement bits")]
        let mut u = v as u128;
        if max_width < 128 {
            u &= (1u128 << max_width) - 1;
        }
        IntRange { width: 128 - u.leading_zeros(), non_negative: true }
    }
}

impl Sema<'_, '_> {
    /// clang's `IntRange::forValueOfType`: an opaque value of integral or
    /// enumeration type `q`; an unscoped enumeration without a fixed
    /// underlying type has the bits of its enumerators.
    pub(crate) fn int_range_of_type(&self, q: QualType) -> IntRange {
        let c = self.ast.types.canonical(q);
        if self.ast.types.builtin(c) == Some(BuiltinType::Bool) {
            return IntRange::BOOL;
        }
        if self.ast.types.as_enum(c).is_some()
            && let Some(e) = enum_data(self, c)
            && !e.is_fixed()
            && let Some(def) = self.ast.types.as_enum(c).and_then(|d| self.ast.definition(d))
        {
            let (mut pos, mut neg) = (0u32, 0u32);
            for m in self.ast.decls.members(def) {
                let Some(v) = self.ast.decls.enumerator(m).and_then(|x| x.value) else { continue };
                let Some(v) = self.const_int(v) else { continue };
                if v < 0 {
                    neg = neg.max(129 - (!v).leading_zeros());
                } else {
                    pos = pos.max(128 - v.leading_zeros());
                }
            }
            return if neg == 0 {
                IntRange { width: pos, non_negative: true }
            } else {
                IntRange { width: (pos + 1).max(neg), non_negative: false }
            };
        }
        let c = match self.ast.types.canonical_view(c) {
            TypeView::Complex(e) | TypeView::Atomic(e) => self.ast.types.canonical(e),
            _ => match self.vector_parts(c) {
                Some(v) => self.ast.types.canonical(v.elem),
                None => c,
            },
        };
        IntRange { width: int_width(self, c).unwrap_or(0), non_negative: !int_signed(self, c) }
    }

    /// clang's `GetExprRange(E, MaxWidth, ..., Approximate = true)` for
    /// integer expression `e`.
    pub(crate) fn expr_int_range(&mut self, e: ExprId, max_width: u32) -> IntRange {
        self.expr_int_range_at(e, max_width, 0)
    }

    fn expr_int_range_at(&mut self, mut e: ExprId, max_width: u32, depth: u32) -> IntRange {
        while self.ast.exprs.get(e).kind() == ExprKind::ParenExpr {
            e = self.ast.exprs.operand(e);
        }
        let n = *self.ast.exprs.get(e);
        let ty = n.ty();
        if depth > MAX_DEPTH {
            return self.int_range_of_type(ty);
        }
        // A full evaluation first.
        if let Some(v) = self.fold_int(e) {
            return IntRange::of_value(v, int_signed(self, ty), max_width);
        }
        let d = depth + 1;
        match n.kind() {
            ExprKind::ImplicitCastExpr => {
                let sub = self.ast.exprs.operand(e);
                let ck = self.ast.exprs.cast_kind(e);
                if matches!(ck, CastKind::NoOp | CastKind::LValueToRValue) {
                    return self.expr_int_range_at(sub, max_width, d);
                }
                let out = self.int_range_of_type(ty);
                if !matches!(ck, CastKind::IntegralCast | CastKind::BooleanToSignedIntegral) {
                    return out;
                }
                let s = self.expr_int_range_at(sub, max_width.min(out.width), d);
                if s.width >= out.width {
                    return out;
                }
                IntRange { width: s.width, non_negative: s.non_negative || out.non_negative }
            }
            ExprKind::ConditionalOperator => {
                let c = self.ast.exprs.children(e, &self.ast.lists)[0];
                let [t, f] = self.ast.exprs.cond_arms(e);
                if let Some(b) = self.try_fold(c).and_then(|v| self.const_truth(v)) {
                    return self.expr_int_range_at(if b { t } else { f }, max_width, d);
                }
                let arm = |s: &mut Self, x: ExprId| {
                    if s.ast.types.builtin(s.ast.exprs.get(x).ty()) == Some(BuiltinType::Void) {
                        IntRange { width: 0, non_negative: true }
                    } else {
                        s.expr_int_range_at(x, max_width, d)
                    }
                };
                let l = arm(self, t);
                let r = arm(self, f);
                IntRange::join(l, r)
            }
            ExprKind::BinaryOperator => {
                let op = self.ast.exprs.binary_op(e);
                let [l, r] = self.ast.exprs.operands(e);
                self.binary_int_range(op, l, r, ty, max_width, d)
            }
            ExprKind::CompoundAssignOperator => {
                let op = self.ast.exprs.binary_op(e);
                let l = self.ast.exprs.children(e, &self.ast.lists)[0];
                let r = self.ast.exprs.compound_assign(e).rhs;
                self.binary_int_range(op, l, r, ty, max_width, d)
            }
            ExprKind::UnaryOperator => match self.ast.exprs.unary_op(e) {
                UnaryOp::LNot => IntRange::BOOL,
                UnaryOp::Deref | UnaryOp::AddrOf => self.int_range_of_type(ty),
                _ => self.expr_int_range_at(self.ast.exprs.operand(e), max_width, d),
            },
            ExprKind::OpaqueValueExpr => match self.ast.exprs.opaque_source(e) {
                Some(s) => self.expr_int_range_at(s, max_width, d),
                None => self.int_range_of_type(ty),
            },
            ExprKind::MemberExpr | ExprKind::DeclRefExpr => {
                match crate::conv::bit_field_width(self, e) {
                    Some(w) => IntRange { width: w, non_negative: !int_signed(self, ty) },
                    None => self.int_range_of_type(ty),
                }
            }
            _ => self.int_range_of_type(ty),
        }
    }

    /// The binary-operator arm of `GetExprRange` (approximate: `+`, `-` and
    /// `*` join their operands' ranges).
    fn binary_int_range(
        &mut self,
        op: BinaryOp,
        l: ExprId,
        r: ExprId,
        ty: QualType,
        max_width: u32,
        d: u32,
    ) -> IntRange {
        let mut combine: fn(IntRange, IntRange) -> IntRange = IntRange::join;
        match op {
            BinaryOp::LAnd
            | BinaryOp::LOr
            | BinaryOp::LT
            | BinaryOp::GT
            | BinaryOp::LE
            | BinaryOp::GE
            | BinaryOp::EQ
            | BinaryOp::NE => return IntRange::BOOL,
            BinaryOp::MulAssign
            | BinaryOp::DivAssign
            | BinaryOp::RemAssign
            | BinaryOp::AddAssign
            | BinaryOp::SubAssign
            | BinaryOp::XorAssign
            | BinaryOp::OrAssign
            | BinaryOp::PtrMemD
            | BinaryOp::PtrMemI
            | BinaryOp::ShlAssign
            | BinaryOp::Cmp => return self.int_range_of_type(ty),
            BinaryOp::Assign | BinaryOp::Comma => return self.expr_int_range_at(r, max_width, d),
            BinaryOp::And | BinaryOp::AndAssign => combine = IntRange::bit_and,
            // `1 << n` is non-negative, an important idiom.
            BinaryOp::Shl => {
                let t = self.int_range_of_type(ty);
                let lit = self.strip_paren_casts(l);
                let one = self.ast.exprs.get(lit).kind() == ExprKind::IntegerLiteral
                    && self.fold_int(lit) == Some(1);
                return IntRange { width: t.width, non_negative: one || t.non_negative };
            }
            BinaryOp::Shr | BinaryOp::ShrAssign => {
                let mut lr = self.expr_int_range_at(l, max_width, d);
                if let Some(s) = self.fold_int(r)
                    && s >= 0
                {
                    match u32::try_from(s) {
                        Ok(s) if s < lr.width => lr.width -= s,
                        _ => lr.width = u32::from(!lr.non_negative),
                    }
                }
                return lr;
            }
            BinaryOp::Sub if self.ast.types.is_pointer(self.ast.exprs.get(l).ty()) => {
                return self.int_range_of_type(ty);
            }
            BinaryOp::Div => {
                let op_w = int_width(self, ty).unwrap_or(max_width);
                let mut lr = self.expr_int_range_at(l, op_w, d);
                if let Some(v) = self.fold_int(r) {
                    // `APSInt::logBase2` of the divisor's bits.
                    let log2 = match v {
                        0 => u32::MAX,
                        v if v > 0 => 127 - v.leading_zeros(),
                        _ => int_width(self, self.ast.exprs.get(r).ty()).unwrap_or(1) - 1,
                    };
                    if log2 >= lr.width {
                        lr.width = u32::from(!lr.non_negative);
                    } else {
                        lr.width = (lr.width - log2).min(max_width);
                    }
                    return lr;
                }
                let rr = self.expr_int_range_at(r, op_w, d);
                return IntRange {
                    width: lr.width,
                    non_negative: lr.non_negative && rr.non_negative,
                };
            }
            BinaryOp::Rem => combine = IntRange::rem,
            _ => {}
        }
        let op_w = int_width(self, ty).unwrap_or(max_width);
        let lr = self.expr_int_range_at(l, op_w, d);
        let rr = self.expr_int_range_at(r, op_w, d);
        let mut c = combine(lr, rr);
        c.non_negative |= !int_signed(self, ty) && int_width(self, ty).is_some();
        c.width = c.width.min(max_width);
        c
    }

    /// clang's `IgnoreParenCasts`: parentheses and every cast.
    fn strip_paren_casts(&self, mut e: ExprId) -> ExprId {
        for _ in 0..256 {
            let k = self.ast.exprs.get(e).kind();
            if k == ExprKind::ParenExpr || k.is_cast() {
                e = self.ast.exprs.operand(e);
            } else {
                break;
            }
        }
        e
    }
}
