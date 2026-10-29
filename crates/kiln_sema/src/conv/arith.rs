//! The usual arithmetic conversions ([expr.arith.conv]), following
//! clang's `UsualArithmeticConversions`.

use kiln_ast::expr::CastKind;
use kiln_ast::{ExprId, QualType};
use kiln_diag::DiagId;

use super::rank::{
    FloatOrder, float_order, int_signed, int_width, integer_rank, is_integral_or_unscoped_enum,
    is_unscoped_enum, promoted_type, unsigned_counterpart,
};
use super::{ConvError, bit_field_width, cast_to, ty, usual_unary_conversions};
use crate::Sema;

/// The operator the usual arithmetic conversions are performed for;
/// selects the wording of the enumeration diagnostics.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ArithConvKind {
    /// A binary arithmetic operator.
    Arithmetic,
    /// A bitwise operator.
    BitwiseOp,
    /// A relational or equality operator.
    Comparison,
    /// The second and third operands of `?:`.
    Conditional,
    /// A compound assignment: the left operand is not converted, but its
    /// type takes part in the conversions promoted.
    CompoundAssign,
}

impl ArithConvKind {
    fn select(self) -> &'static str {
        match self {
            ArithConvKind::Arithmetic => "0",
            ArithConvKind::BitwiseOp => "1",
            ArithConvKind::Comparison => "2",
            ArithConvKind::Conditional => "3",
            ArithConvKind::CompoundAssign => "4",
        }
    }
}

/// The type both `a` and `b` desugar to that keeps the most sugar: the
/// first node of `a`'s desugaring chain on `b`'s, else the canonical
/// type.
fn common_sugar(s: &Sema<'_, '_>, a: QualType, b: QualType) -> QualType {
    let chain = |mut q: QualType| {
        let mut v = vec![q];
        for _ in 0..256 {
            let n = s.ast.types.desugar_once(q);
            if n == q {
                break;
            }
            v.push(n);
            q = n;
        }
        v
    };
    let (ca, cb) = (chain(a), chain(b));
    ca.iter().find(|q| cb.contains(q)).copied().unwrap_or_else(|| s.ast.types.canonical(a))
}

/// The type the left operand `l` (of type `lt`, cv-unqualified) of a
/// compound assignment takes part in the usual arithmetic conversions
/// with: its integral promotion ([conv.prom]), with the bit-field
/// promotion of /5 for a bit-field, else `lt` itself. clang:
/// `isPromotableIntegerType` / `isPromotableBitField` in
/// `UsualArithmeticConversions`.
fn compound_lhs_type(s: &Sema<'_, '_>, l: ExprId, lt: QualType) -> QualType {
    if !is_integral_or_unscoped_enum(s, lt) {
        return lt;
    }
    promoted_type(s, lt, bit_field_width(s, l)).unwrap_or(lt)
}

/// The usual arithmetic conversions ([expr.arith.conv]) on the operands
/// `l` and `r` of an operator of kind `kind`: the usual unary
/// conversions, then conversion to the common type, which is returned
/// with the converted operands. Two operands of the same type (including
/// a scoped enumeration) keep it.
///
/// For a compound assignment ([expr.ass]/6, `E1 op= E2` is
/// `E1 = E1 op E2`) the left operand is returned unconverted, but the
/// common type is computed from its *promoted* type ([conv.prom],
/// including the bit-field promotion of /5), as clang's
/// `UsualArithmeticConversions` does: two operands of the same
/// unpromoted type keep it (`unsigned c : 9; c += 2u` computes in
/// `unsigned int`); otherwise the left type is promoted first
/// (`c += 2` computes in `int`, `char32_t x; x += 1` in `unsigned int`).
///
/// # Errors
/// - an unscoped enumeration with a floating-point type, or two
///   different unscoped enumerations (ill-formed since C++26,
///   [expr.arith.conv]/1.2–1.3; clang's wording);
/// - two floating-point types with unordered ranks;
/// - non-arithmetic operands: `sema_invalid_operands` with the operand
///   types as written.
pub fn usual_arithmetic_conversions_kind(
    s: &mut Sema<'_, '_>,
    l: ExprId,
    r: ExprId,
    kind: ArithConvKind,
) -> Result<(ExprId, ExprId, QualType), ConvError> {
    let (lt0, rt0) = (ty(s, l), ty(s, r));
    if s.ast.types.is_dependent(lt0) || s.ast.types.is_dependent(rt0) {
        return Ok((l, r, QualType::DEPENDENT));
    }
    let err = |q: QualType| s.ast.types.canonical(q).without_quals() == QualType::ERROR;
    if err(lt0) || err(rt0) {
        return Ok((l, r, QualType::ERROR));
    }
    let ca = kind == ArithConvKind::CompoundAssign;
    let (le, re) = (is_unscoped_enum(s, lt0), is_unscoped_enum(s, rt0));
    let (lf, rf) = (s.ast.types.is_floating(lt0), s.ast.types.is_floating(rt0));
    if (!ca && le && rf) || (re && lf) {
        return Err(ConvError::new(
            DiagId::sema_arith_enum_float,
            vec![
                kind.select().into(),
                if le { "1" } else { "0" }.into(),
                s.ty_arg(lt0),
                s.ty_arg(rt0),
            ],
        ));
    }
    if !ca && le && re && !s.ast.types.same_unqualified(lt0, rt0) {
        return Err(ConvError::new(
            DiagId::sema_arith_enum_enum,
            vec![kind.select().into(), s.ty_arg(lt0), s.ty_arg(rt0)],
        ));
    }
    let l = if ca { l } else { usual_unary_conversions(s, l) };
    let r = usual_unary_conversions(s, r);
    let (lt, rt) = (super::strip_cv(s, ty(s, l)), super::strip_cv(s, ty(s, r)));
    if s.ast.types.same_unqualified(lt, rt) {
        return Ok((l, r, common_sugar(s, lt, rt)));
    }
    // The left operand of a compound assignment is promoted in type only
    // (the other kinds' left operands were promoted above). An unscoped
    // enumeration is arithmetic here through its promotion, as in clang.
    let lt = if ca { compound_lhs_type(s, l, lt) } else { lt };
    let arith = |q: QualType| s.ast.types.is_arithmetic(q);
    if !arith(lt) || !arith(rt) {
        return Err(ConvError::new(
            DiagId::sema_invalid_operands,
            vec![s.ty_arg(lt0), s.ty_arg(rt0)],
        ));
    }
    if ca && s.ast.types.same_unqualified(lt, rt) {
        return Ok((l, r, common_sugar(s, lt, rt)));
    }
    let (lfl, rfl) = (s.ast.types.is_floating(lt), s.ast.types.is_floating(rt));
    if lfl && rfl {
        let (lb, rb) = (s.ast.types.builtin(lt), s.ast.types.builtin(rt));
        let order = match (lb, rb) {
            (Some(a), Some(b)) => float_order(s, a, b),
            _ => FloatOrder::Unordered,
        };
        return match order {
            FloatOrder::Unordered => Err(ConvError::new(
                DiagId::sema_arith_float_unordered,
                vec![kind.select().into(), s.ty_arg(lt), s.ty_arg(rt)],
            )),
            FloatOrder::Greater | FloatOrder::Equal => {
                let r = cast_to(s, CastKind::FloatingCast, r, lt, None);
                Ok((l, r, lt))
            }
            FloatOrder::Less => {
                let l = if ca { l } else { cast_to(s, CastKind::FloatingCast, l, rt, None) };
                Ok((l, r, rt))
            }
        };
    }
    if lfl {
        let r = cast_to(s, CastKind::IntegralToFloating, r, lt, None);
        return Ok((l, r, lt));
    }
    if rfl {
        let l = if ca { l } else { cast_to(s, CastKind::IntegralToFloating, l, rt, None) };
        return Ok((l, r, rt));
    }
    // Both integral ([expr.arith.conv]/1.5).
    let (lr, rr) = (integer_rank(s, lt).unwrap_or(0), integer_rank(s, rt).unwrap_or(0));
    let (ls, rs) = (int_signed(s, lt), int_signed(s, rt));
    let cast_r =
        |s: &mut Sema<'_, '_>, to: QualType| cast_to(s, CastKind::IntegralCast, r, to, None);
    let cast_l = |s: &mut Sema<'_, '_>, to: QualType| {
        if ca { l } else { cast_to(s, CastKind::IntegralCast, l, to, None) }
    };
    let use_l = |s: &mut Sema<'_, '_>| (l, cast_r(s, lt), lt);
    let use_r = |s: &mut Sema<'_, '_>| (cast_l(s, rt), r, rt);
    let out = if ls == rs {
        if lr >= rr { use_l(s) } else { use_r(s) }
    } else if (ls && rr >= lr) || (rs && lr >= rr) {
        // The unsigned type's rank is not less than the signed one's.
        if rs { use_l(s) } else { use_r(s) }
    } else if int_width(s, lt) != int_width(s, rt) {
        // The signed type can hold every value of the unsigned one.
        if ls { use_l(s) } else { use_r(s) }
    } else {
        let u = unsigned_counterpart(s, if ls { lt } else { rt });
        let r2 = cast_r(s, u);
        let l2 = cast_l(s, u);
        (l2, r2, u)
    };
    Ok(out)
}
