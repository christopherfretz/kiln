//! Narrowing conversions ([dcl.init.list]/7), following clang's
//! `StandardConversionSequence::getNarrowingKind`.

use kiln_ast::consts::{FloatKind, Scalar};
use kiln_ast::expr::{CastKind, ExprKind, UnaryOp};
use kiln_ast::types::{BuiltinType, TypeView};
use kiln_ast::{ConstValueId, ExprId, QualType};
use kiln_diag::{Diag, DiagId};

use super::rank::{builtin_width, enum_data, int_signed, int_width, is_integral_or_unscoped_enum};
use super::scs::{SecondStep, StandardConversion};
use super::{bit_field_width, soft, ty};
use crate::Sema;

/// Whether a standard conversion is narrowing.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Narrowing {
    /// Not narrowing.
    NotNarrowing,
    /// Narrowing by type, whatever the value ([dcl.init.list]/7.1, 7.5).
    Type,
    /// Narrowing because the source is not a constant expression.
    Variable,
    /// The constant source's value does not survive: its value, as
    /// clang prints it.
    Constant {
        /// The value.
        value: String,
    },
    /// Cannot tell: the source is value-dependent.
    Dependent,
}

/// A constant source value.
#[derive(Clone, Copy, Debug)]
enum Val {
    /// An integer: sign and magnitude.
    Int { neg: bool, mag: u128 },
    /// A floating-point value.
    Float { kind: FloatKind, bits: u128 },
}

fn scalar_val(s: &Sema<'_, '_>, v: ConstValueId) -> Option<Val> {
    match s.ast.consts.scalar(v)? {
        Scalar::Int { signed, bits, .. } => {
            if signed {
                let i = bits.to_i128()?;
                Some(Val::Int { neg: i < 0, mag: i.unsigned_abs() })
            } else {
                Some(Val::Int { neg: false, mag: bits.to_u128()? })
            }
        }
        Scalar::Bool { value, .. } => Some(Val::Int { neg: false, mag: u128::from(value) }),
        Scalar::Float { kind, bits, .. } => Some(Val::Float { kind, bits }),
        _ => None,
    }
}

/// The value of a literal initializer, through parentheses, implicit
/// casts that keep the value (`NoOp`, `LValueToRValue`), `ConstantExpr`
/// and unary `+`/`-`.
fn literal_val(s: &Sema<'_, '_>, e: ExprId) -> Option<Val> {
    let n = s.ast.exprs.get(e);
    match n.kind() {
        ExprKind::IntegerLiteral => match s.ast.exprs.int_value(e) {
            Some(v) => Some(Val::Int { neg: false, mag: u128::from(v) }),
            None => scalar_val(s, s.ast.exprs.literal_const(e)?),
        },
        ExprKind::CharacterLiteral => {
            let v = s.ast.exprs.char_value(e);
            // An ordinary character literal of a signed `char` has the
            // value of the sign-extended byte.
            let t = n.ty();
            if int_signed(s, t) {
                let w = int_width(s, t).unwrap_or(32).min(32);
                let sh = 32 - w;
                let i = ((v << sh) as i32) >> sh;
                Some(Val::Int { neg: i < 0, mag: u128::from(i.unsigned_abs()) })
            } else {
                Some(Val::Int { neg: false, mag: u128::from(v) })
            }
        }
        ExprKind::CXXBoolLiteralExpr => {
            Some(Val::Int { neg: false, mag: u128::from(s.ast.exprs.bool_value(e)) })
        }
        ExprKind::FloatingLiteral => scalar_val(s, s.ast.exprs.literal_const(e)?),
        ExprKind::ParenExpr => literal_val(s, s.ast.exprs.operand(e)),
        ExprKind::ConstantExpr => match s.ast.exprs.constant_value(e) {
            Some(v) => scalar_val(s, v),
            None => literal_val(s, s.ast.exprs.operand(e)),
        },
        ExprKind::ImplicitCastExpr if matches!(s.ast.exprs.cast_kind(e), CastKind::NoOp) => {
            literal_val(s, s.ast.exprs.operand(e))
        }
        ExprKind::UnaryOperator => {
            let op = s.ast.exprs.unary_op(e);
            let v = literal_val(s, s.ast.exprs.operand(e))?;
            match op {
                UnaryOp::Plus => Some(v),
                UnaryOp::Minus => Some(match v {
                    Val::Int { neg, mag } => {
                        // Negation in the operand's (promoted) type.
                        let t = n.ty();
                        if int_signed(s, t) {
                            Val::Int { neg: !neg && mag != 0, mag }
                        } else {
                            let w = int_width(s, t).unwrap_or(128);
                            let m = if w >= 128 { u128::MAX } else { (1u128 << w) - 1 };
                            Val::Int { neg: false, mag: (0u128.wrapping_sub(mag)) & m }
                        }
                    }
                    Val::Float { kind, bits } => Val::Float { kind, bits: bits ^ sign_mask(kind) },
                }),
                _ => None,
            }
        }
        _ => None,
    }
}

fn sign_mask(kind: FloatKind) -> u128 {
    let total = match kind {
        FloatKind::Half | FloatKind::BFloat16 => 16,
        FloatKind::Single => 32,
        FloatKind::Double => 64,
        FloatKind::X87Extended => 80,
        FloatKind::Quad => 128,
    };
    1u128 << (total - 1)
}

/// The `FloatKind` of a floating builtin on the target.
pub(crate) fn float_kind(s: &Sema<'_, '_>, b: BuiltinType) -> Option<FloatKind> {
    use kiln_ast::types::FloatFormat as F;
    Some(match super::rank::float_format(s, b)? {
        F::IeeeHalf => FloatKind::Half,
        F::BFloat => FloatKind::BFloat16,
        F::IeeeSingle => FloatKind::Single,
        F::IeeeDouble | F::PpcDoubleDouble => FloatKind::Double,
        F::X87Extended => FloatKind::X87Extended,
        F::IeeeQuad => FloatKind::Quad,
    })
}

/// clang's floating-point rank (`getFloatingRank`), used for the
/// narrowing test between floating types.
fn clang_float_rank(b: BuiltinType) -> u8 {
    match b {
        BuiltinType::BFloat16 => 0,
        BuiltinType::Float16 => 1,
        BuiltinType::Half => 2,
        BuiltinType::Float => 3,
        BuiltinType::Double => 4,
        BuiltinType::LongDouble => 5,
        _ => 6,
    }
}

fn int_string(neg: bool, mag: u128) -> String {
    if neg { format!("-{mag}") } else { mag.to_string() }
}

/// True if the integer `neg`/`mag` is a value of a `w`-bit integer of
/// signedness `signed`.
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

/// Classifies the conversion `scs` of the initializer `init` as
/// narrowing or not ([dcl.init.list]/7). `value` is the initializer's
/// value if it is a constant expression the caller has evaluated;
/// literals (possibly negated or parenthesized) are evaluated here.
pub fn narrowing_kind(
    s: &Sema<'_, '_>,
    scs: &StandardConversion,
    init: ExprId,
    value: Option<ConstValueId>,
) -> Narrowing {
    let from = scs.to_types[0];
    let mut to = scs.to_types[1];
    if s.ast.types.as_enum(to).is_some()
        && let Some(u) = enum_data(s, to).and_then(|e| e.int_type)
    {
        to = u;
    }
    let dependent = s.ast.exprs.get(init).is_value_dependent();
    let val = || value.and_then(|v| scalar_val(s, v)).or_else(|| literal_val(s, init));
    let mut step = scs.second;
    if step == SecondStep::BooleanConversion {
        step = if s.ast.types.is_floating(from) {
            SecondStep::FloatingIntegral
        } else if is_integral_or_unscoped_enum(s, from) {
            SecondStep::IntegralConversion
        } else {
            return Narrowing::Type;
        };
    }
    match step {
        SecondStep::FloatingIntegral => {
            if s.ast.types.is_floating(from) {
                return Narrowing::Type;
            }
            // Integer to floating ([dcl.init.list]/7.3).
            if dependent {
                return Narrowing::Dependent;
            }
            let Some(Val::Int { neg, mag }) = val() else { return Narrowing::Variable };
            let Some(kind) = s.ast.types.builtin(to).and_then(|b| float_kind(s, b)) else {
                return Narrowing::NotNarrowing;
            };
            let r = soft::from_int(kind, neg, mag);
            // Back to the source type, rounding toward zero and
            // saturating.
            let (w, sg) = (int_width(s, from).unwrap_or(64), int_signed(s, from));
            let back = match soft::to_int(kind, r.bits) {
                Some((n, m, _)) => clamp(n, m, w, sg),
                None => clamp(neg, u128::MAX, w, sg),
            };
            if r.overflow || back != (neg && mag != 0, mag) {
                Narrowing::Constant { value: int_string(neg, mag) }
            } else {
                Narrowing::NotNarrowing
            }
        }
        SecondStep::FloatingConversion => {
            let (Some(fb), Some(tb)) = (s.ast.types.builtin(from), s.ast.types.builtin(to)) else {
                return Narrowing::NotNarrowing;
            };
            if clang_float_rank(fb) <= clang_float_rank(tb) {
                return Narrowing::NotNarrowing;
            }
            if dependent {
                return Narrowing::Dependent;
            }
            let Some(Val::Float { kind, bits }) = val() else { return Narrowing::Variable };
            let Some(tk) = float_kind(s, tb) else { return Narrowing::NotNarrowing };
            let r = soft::convert(kind, tk, bits);
            if r.overflow {
                Narrowing::Constant { value: soft::format_e6(kind, bits) }
            } else {
                Narrowing::NotNarrowing
            }
        }
        SecondStep::IntegralConversion | SecondStep::IntegralPromotion => {
            let mut fw = int_width(s, from).unwrap_or(0);
            if let Some(bw) = bit_field_width(s, init) {
                fw = fw.min(bw);
            }
            let fs = int_signed(s, from);
            let (tw, ts) = match s.ast.types.builtin(to) {
                Some(BuiltinType::Bool) => (1, false),
                Some(b) => (builtin_width(s, b), int_signed(s, to)),
                None => (int_width(s, to).unwrap_or(0), int_signed(s, to)),
            };
            if !(fw > tw || (fw == tw && fs != ts) || (fs && !ts)) {
                return Narrowing::NotNarrowing;
            }
            if dependent {
                return Narrowing::Dependent;
            }
            match val() {
                Some(Val::Int { neg, mag }) => {
                    if representable(neg, mag, tw, ts) {
                        Narrowing::NotNarrowing
                    } else {
                        Narrowing::Constant { value: int_string(neg, mag) }
                    }
                }
                _ => Narrowing::Variable,
            }
        }
        _ => Narrowing::NotNarrowing,
    }
}

/// Saturates sign/magnitude to a `w`-bit integer of signedness `signed`
/// (as sign/magnitude, with zero non-negative).
fn clamp(neg: bool, mag: u128, w: u32, signed: bool) -> (bool, u128) {
    if mag == 0 {
        return (false, 0);
    }
    if signed {
        let lim = if w >= 128 { 1u128 << 127 } else { 1u128 << (w - 1) };
        if neg { (true, mag.min(lim)) } else { (false, mag.min(lim - 1)) }
    } else if neg {
        (false, 0)
    } else {
        let max = if w >= 128 { u128::MAX } else { (1u128 << w) - 1 };
        (false, mag.min(max))
    }
}

/// The diagnostic for a narrowing conversion of `init` to `to` (with
/// its "insert an explicit cast" note), or `None` if `n` is not
/// narrowing.
pub fn narrowing_diag(s: &Sema<'_, '_>, n: &Narrowing, init: ExprId, to: QualType) -> Option<Diag> {
    let loc = s.begin_loc(init);
    let to_arg = s.ty_arg(super::strip_cv(s, to));
    let d = match n {
        Narrowing::NotNarrowing | Narrowing::Dependent => return None,
        Narrowing::Type => s
            .diag(DiagId::sema_init_list_type_narrowing, loc)
            .arg(s.ty_arg(ty(s, init).without_quals()))
            .arg(to_arg),
        Narrowing::Variable => s
            .diag(DiagId::sema_init_list_variable_narrowing, loc)
            .arg(s.ty_arg(ty(s, init).without_quals()))
            .arg(to_arg),
        Narrowing::Constant { value } => {
            s.diag(DiagId::sema_init_list_constant_narrowing, loc).arg(value).arg(to_arg)
        }
    };
    // clang offers the `static_cast<T>(...)` fix-it (and its note) only
    // when `T` can be spelled: a typedef or a builtin type.
    if !spellable(s, to) {
        return Some(d);
    }
    Some(d.with_note(s.diag(DiagId::note_init_list_narrowing_silence, loc)))
}

/// True if `t` (through elaborated and using sugar) is a typedef or a
/// builtin type (clang's `getAs<TypedefType>`/`getAs<BuiltinType>`).
fn spellable(s: &Sema<'_, '_>, mut t: QualType) -> bool {
    for _ in 0..64 {
        match s.ast.types.view(t.ty()) {
            TypeView::Elaborated { named, .. } => t = named,
            TypeView::Using { underlying, .. } => t = underlying,
            TypeView::Typedef { .. } | TypeView::Builtin(_) => return true,
            _ => return false,
        }
    }
    false
}
