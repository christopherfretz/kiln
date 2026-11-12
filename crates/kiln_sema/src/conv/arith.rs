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
    // The chain of `q`: `q`, then each `desugar_once` step (at most 256),
    // walked in place rather than collected (WP-645).
    let on_chain = |mut q: QualType, target: QualType| {
        if q == target {
            return true;
        }
        for _ in 0..256 {
            let n = s.ast.types.desugar_once(q);
            if n == q {
                break;
            }
            if n == target {
                return true;
            }
            q = n;
        }
        false
    };
    let mut q = a;
    if on_chain(b, q) {
        return q;
    }
    for _ in 0..256 {
        let n = s.ast.types.desugar_once(q);
        if n == q {
            break;
        }
        if on_chain(b, n) {
            return n;
        }
        q = n;
    }
    s.ast.types.canonical(a)
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

/// The enumeration operands of the usual arithmetic conversions
/// ([expr.arith.conv]/1.2–1.3, clang's wording): an unscoped enumeration
/// with a floating type, or two different enumerations. Ill-formed since
/// C++26 (P2864; the error, the deprecation warning under `-fpermissive`),
/// deprecated in C++20 and C++23 (`Ok` with
/// clang's `-Wdeprecated-*` warning, for the caller to report at the
/// operator), and fine before (`Ok(None)`: clang's `-Wenum-*-conversion`
/// warnings are off by default).
fn check_enum_operands(
    s: &Sema<'_, '_>,
    lt0: QualType,
    rt0: QualType,
    kind: ArithConvKind,
) -> Result<Option<ConvError>, ConvError> {
    let ca = kind == ArithConvKind::CompoundAssign;
    let (le, re) = (is_unscoped_enum(s, lt0), is_unscoped_enum(s, rt0));
    let (lf, rf) = (s.ast.types.is_floating(lt0), s.ast.types.is_floating(rt0));
    // `-fpermissive` makes the C++26 error the C++20 deprecation warning.
    let ill_formed = s.std >= kiln_lex::Std::Cxx26 && !s.permissive;
    let deprecated = !ill_formed && s.std >= kiln_lex::Std::Cxx20;
    if (!ca && le && rf) || (re && lf) {
        let id = if ill_formed {
            DiagId::sema_arith_enum_float
        } else if deprecated {
            DiagId::sema_arith_enum_float_deprecated
        } else {
            return Ok(None);
        };
        let e = ConvError::new(
            id,
            vec![
                kind.select().into(),
                if le { "1" } else { "0" }.into(),
                s.ty_arg(lt0),
                s.ty_arg(rt0),
            ],
        );
        return if ill_formed { Err(e) } else { Ok(Some(e)) };
    }
    if !ca && le && re && !s.ast.types.same_unqualified(lt0, rt0) {
        let args = vec![kind.select().into(), s.ty_arg(lt0), s.ty_arg(rt0)];
        if ill_formed {
            return Err(ConvError::new(DiagId::sema_arith_enum_enum, args));
        }
        if !deprecated {
            return Ok(None);
        }
        // clang `checkEnumArithmeticConversions`: an enumeration without a
        // name for linkage on either side has its own group, whatever the
        // operator.
        let named = |q: QualType| {
            s.ast.types.as_enum(q).is_some_and(|d| {
                !s.ast.decls.get(d).name().is_empty() || s.ast.decls.typedef_for_anon(d).is_some()
            })
        };
        let id = match kind {
            _ if !named(lt0) || !named(rt0) => DiagId::sema_arith_anon_enum_enum_deprecated,
            ArithConvKind::Comparison => DiagId::sema_compare_enum_enum_deprecated,
            ArithConvKind::Conditional => DiagId::sema_conditional_enum_enum_deprecated,
            _ => DiagId::sema_arith_enum_enum_deprecated,
        };
        return Ok(Some(ConvError::new(id, args)));
    }
    Ok(None)
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
    s.arith_warning = None;
    let (lt0, rt0) = (ty(s, l), ty(s, r));
    if s.ast.types.is_dependent(lt0) || s.ast.types.is_dependent(rt0) {
        return Ok((l, r, QualType::DEPENDENT));
    }
    let err = |q: QualType| s.ast.types.canonical(q).without_quals() == QualType::ERROR;
    if err(lt0) || err(rt0) {
        return Ok((l, r, QualType::ERROR));
    }
    let ca = kind == ArithConvKind::CompoundAssign;
    s.arith_warning = check_enum_operands(s, lt0, rt0, kind)?;
    let l = if ca { l } else { usual_unary_conversions(s, l) };
    let r = usual_unary_conversions(s, r);
    let (lt, rt) = (super::strip_cv(s, ty(s, l)), super::strip_cv(s, ty(s, r)));
    // clang: "for conversion purposes, we ignore any atomic qualifier on
    // the LHS operand" (an `_Atomic` compound-assignment target).
    let lt = s.ast.types.as_atomic(lt).unwrap_or(lt);
    if s.ast.types.same_unqualified(lt, rt) {
        return Ok((l, r, common_sugar(s, lt, rt)));
    }
    // The left operand of a compound assignment is promoted in type only
    // (the other kinds' left operands were promoted above). An unscoped
    // enumeration is arithmetic here through its promotion, as in clang.
    let lt = if ca { compound_lhs_type(s, l, lt) } else { lt };
    let arith = |q: QualType| s.ast.types.is_arithmetic(q) || s.ast.types.as_complex(q).is_some();
    if !arith(lt) || !arith(rt) {
        return Err(ConvError::new(
            DiagId::sema_invalid_operands,
            vec![s.ty_arg(lt0), s.ty_arg(rt0)],
        ));
    }
    if ca && s.ast.types.same_unqualified(lt, rt) {
        return Ok((l, r, common_sugar(s, lt, rt)));
    }
    if s.ast.types.as_complex(lt).is_some() || s.ast.types.as_complex(rt).is_some() {
        return complex_conversions(s, l, r, lt, rt, kind);
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
        // clang `handleFloatConversion`: a (compound-assignment) `__fp16`
        // left operand computes in float with an integer right operand.
        let lt = if s.ast.types.builtin(s.ast.types.canonical(lt))
            == Some(kiln_ast::types::BuiltinType::Half)
        {
            QualType::FLOAT
        } else {
            lt
        };
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

/// The common type of two (promoted) integer types ([expr.arith.conv]/1.5).
fn int_common(s: &mut Sema<'_, '_>, lt: QualType, rt: QualType) -> QualType {
    if s.ast.types.same_unqualified(lt, rt) {
        return lt;
    }
    let (lr, rr) = (integer_rank(s, lt).unwrap_or(0), integer_rank(s, rt).unwrap_or(0));
    let (ls, rs) = (int_signed(s, lt), int_signed(s, rt));
    if ls == rs {
        if lr >= rr { lt } else { rt }
    } else if (ls && rr >= lr) || (rs && lr >= rr) {
        if rs { lt } else { rt }
    } else if int_width(s, lt) != int_width(s, rt) {
        if ls { lt } else { rt }
    } else {
        unsigned_counterpart(s, if ls { lt } else { rt })
    }
}

/// The usual arithmetic conversions with a GNU complex operand (clang's
/// `handleComplexConversion`, the complex arm of `handleFloatConversion`
/// and `handleComplexIntConversion`). A real floating operand keeps its
/// realness (only its precision is promoted); an integer operand of a
/// complex floating type, and a real operand of a complex integer type,
/// become complex. The left operand of a compound assignment is left
/// unconverted.
fn complex_conversions(
    s: &mut Sema<'_, '_>,
    l: ExprId,
    r: ExprId,
    lt: QualType,
    rt: QualType,
    kind: ArithConvKind,
) -> Result<(ExprId, ExprId, QualType), ConvError> {
    let ca = kind == ArithConvKind::CompoundAssign;
    let (lc, rc) = (s.ast.types.as_complex(lt), s.ast.types.as_complex(rt));
    let fl = |s: &Sema<'_, '_>, t: QualType| s.ast.types.is_floating(t);
    let elem = |t: QualType, c: Option<QualType>| c.unwrap_or(t);
    let (le, re) = (elem(lt, lc), elem(rt, rc));
    if !fl(s, le) && !fl(s, re) {
        // Complex integers: the elements' common integer type.
        let common = int_common(s, le, re);
        let ct = s.ast.types.complex(common);
        let conv = |s: &mut Sema<'_, '_>, e: ExprId, t: QualType, complex: bool| {
            let e = if s.ast.types.same_unqualified(t, common) {
                e
            } else {
                let (ck, to) = if complex {
                    (CastKind::IntegralComplexCast, ct)
                } else {
                    (CastKind::IntegralCast, common)
                };
                cast_to(s, ck, e, to, None)
            };
            if complex { e } else { cast_to(s, CastKind::IntegralRealToComplex, e, ct, None) }
        };
        let l = if ca { l } else { conv(s, l, le, lc.is_some()) };
        let r = conv(s, r, re, rc.is_some());
        return Ok((l, r, ct));
    }
    // An integer operand converts to the other side's element type and a
    // complex integer one to its complex floating type (clang
    // `handleIntegerToComplexFloatConversion`); with a real floating
    // other side both become the complex of that type
    // (`handleIntToFloatConversion`). The result keeps the complex
    // floating operand's type as written.
    let to_float = |s: &mut Sema<'_, '_>, e: ExprId, c: bool, f: QualType, ct: QualType| {
        if c {
            cast_to(s, CastKind::IntegralComplexToFloatingComplex, e, ct, None)
        } else {
            cast_to(s, CastKind::IntegralToFloating, e, f, None)
        }
    };
    if !fl(s, re) {
        let ct = if lc.is_some() { lt } else { s.ast.types.complex(le) };
        let l = if lc.is_none() && !ca {
            cast_to(s, CastKind::FloatingRealToComplex, l, ct, None)
        } else {
            l
        };
        return Ok((l, to_float(s, r, rc.is_some(), le, ct), ct));
    }
    if !fl(s, le) {
        let ct = if rc.is_some() { rt } else { s.ast.types.complex(re) };
        let r =
            if rc.is_none() { cast_to(s, CastKind::FloatingRealToComplex, r, ct, None) } else { r };
        let l = if ca { l } else { to_float(s, l, lc.is_some(), re, ct) };
        return Ok((l, r, ct));
    }
    // Both floating: the more precise element type wins; the other side
    // is promoted in precision only (clang `handleComplexFloatConversion`:
    // the result is the longer operand's type if it is complex).
    let order = match (s.ast.types.builtin(le), s.ast.types.builtin(re)) {
        (Some(a), Some(b)) => float_order(s, a, b),
        _ => FloatOrder::Unordered,
    };
    let promote = |s: &mut Sema<'_, '_>, e: ExprId, c: bool, f: QualType, ct: QualType| {
        if c {
            cast_to(s, CastKind::FloatingComplexCast, e, ct, None)
        } else {
            cast_to(s, CastKind::FloatingCast, e, f, None)
        }
    };
    match order {
        FloatOrder::Unordered => Err(ConvError::new(
            DiagId::sema_arith_float_unordered,
            vec![kind.select().into(), s.ty_arg(lt), s.ty_arg(rt)],
        )),
        FloatOrder::Less => {
            let ct = if rc.is_some() { rt } else { s.ast.types.complex(re) };
            let l = if ca { l } else { promote(s, l, lc.is_some(), re, ct) };
            Ok((l, r, ct))
        }
        FloatOrder::Equal => Ok((l, r, if lc.is_some() { lt } else { s.ast.types.complex(le) })),
        FloatOrder::Greater => {
            let ct = if lc.is_some() { lt } else { s.ast.types.complex(le) };
            let r = promote(s, r, rc.is_some(), le, ct);
            Ok((l, r, ct))
        }
    }
}
