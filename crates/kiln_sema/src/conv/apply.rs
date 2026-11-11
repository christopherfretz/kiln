//! Performing a standard conversion sequence: the `ImplicitCastExpr`s
//! clang's `PerformImplicitConversion` builds for each step.

use kiln_ast::expr::{CastKind, ValueCategory};
use kiln_ast::types::BuiltinType;
use kiln_ast::{ExprId, QualType};
use kiln_diag::DiagId;

use super::bases::{BaseSearch, ambiguous_paths_string, find_base, path_list};
use super::rank::is_integral_or_unscoped_enum;
use super::scs::{
    FirstStep, SecondStep, StandardConversion, ThirdStep, class_decl, member_pointer_parts,
};
use super::{ConvError, cast_to, decay, lvalue_to_rvalue, strip_cv, ty};
use crate::Sema;

/// Applies the standard conversion sequence `scs` (computed by
/// [`super::standard_conversion`] for `e` and `to`) to `e`, returning the
/// converted prvalue.
///
/// Cast targets follow clang: the second conversion casts to `to` as
/// written; a qualification conversion adds a `NoOp` to `to` without
/// top-level qualifiers only when that is a different type; identity
/// steps add nothing.
///
/// # Errors
/// An ambiguous derived-to-base pointer conversion
/// (`sema_ambiguous_derived_to_base`), an ambiguous or virtual-base
/// pointer-to-member conversion (`sema_ambiguous_member_pointer_conv`,
/// `sema_member_pointer_conv_via_virtual`).
pub fn apply_standard_conversion(
    s: &mut Sema<'_, '_>,
    e: ExprId,
    scs: &StandardConversion,
    to: QualType,
) -> Result<ExprId, ConvError> {
    let mut e = e;
    // A sequence to `_Atomic(T)` that ends at `T` (clang's
    // `tryAtomicConversion`): convert to `T`, then `NonAtomicToAtomic`.
    let atomic_to = s
        .ast
        .types
        .as_atomic(to)
        .filter(|_| !s.ast.types.same_unqualified(scs.to_types[2], to))
        .map(|v| (to, v));
    let to = atomic_to.map_or(to, |(_, v)| v);
    match scs.first {
        FirstStep::Identity => {}
        FirstStep::LValueToRValue => {
            let nullptr_src = s.ast.types.builtin(scs.from) == Some(BuiltinType::NullPtr);
            let null_conv = matches!(
                scs.second,
                SecondStep::PointerConversion | SecondStep::MemberPointerConversion
            );
            if !(nullptr_src && null_conv) {
                e = lvalue_to_rvalue(s, e);
            }
        }
        FirstStep::ArrayToPointer | FirstStep::FunctionToPointer => e = decay(s, e),
    }
    e = apply_second(s, e, scs, to)?;
    match scs.third {
        ThirdStep::Identity => {}
        ThirdStep::Qualification => {
            let u = s.ast.types.unqualified(to);
            e = cast_to(s, CastKind::NoOp, e, u, None);
        }
        ThirdStep::FunctionPointer => e = cast_to(s, CastKind::NoOp, e, to, None),
    }
    if let Some((at, _)) = atomic_to {
        let at = s.ast.types.unqualified(at);
        e = cast_to(s, CastKind::NonAtomicToAtomic, e, at, None);
    }
    Ok(e)
}

fn apply_second(
    s: &mut Sema<'_, '_>,
    e: ExprId,
    scs: &StandardConversion,
    to: QualType,
) -> Result<ExprId, ConvError> {
    let from = ty(s, e);
    let to_bool = s.ast.types.builtin(to) == Some(BuiltinType::Bool);
    Ok(match scs.second {
        SecondStep::Identity => e,
        SecondStep::VectorSplat => {
            let x = s.prepare_vector_splat(to, e);
            let u = s.ast.types.unqualified(to);
            super::raw_cast(s, CastKind::VectorSplat, x, u, ValueCategory::PRValue, None)
        }
        SecondStep::VectorConversion => {
            let u = s.ast.types.unqualified(to);
            cast_to(s, CastKind::BitCast, e, u, None)
        }
        SecondStep::IntegralPromotion | SecondStep::IntegralConversion => {
            let ck = if to_bool { CastKind::IntegralToBoolean } else { CastKind::IntegralCast };
            cast_to(s, ck, e, to, None)
        }
        SecondStep::FloatingPromotion => cast_to(s, CastKind::FloatingCast, e, to, None),
        SecondStep::FloatingConversion => {
            if !s.explicit_conv {
                fp_rank_check(s, e, from, to);
            }
            cast_to(s, CastKind::FloatingCast, e, to, None)
        }
        SecondStep::FloatingIntegral => {
            let ck = if s.ast.types.is_floating(from) {
                CastKind::FloatingToIntegral
            } else {
                CastKind::IntegralToFloating
            };
            cast_to(s, ck, e, to, None)
        }
        SecondStep::ComplexPromotion | SecondStep::ComplexConversion => {
            let ck = complex_cast_kind(s, from, to);
            cast_to(s, ck, e, to, None)
        }
        SecondStep::ComplexReal => return complex_real(s, e, from, to),
        SecondStep::BooleanConversion => boolean_conversion(s, e, from, to),
        SecondStep::PointerConversion => pointer_conversion(s, e, scs, from, to)?,
        SecondStep::DerivedToBase => derived_to_base(s, e, from, to)?,
        SecondStep::MemberPointerConversion => member_pointer_conversion(s, e, scs, from, to)?,
    })
}

/// [conv.bool]: the cast to `bool` by the source type's kind (a `__fp16`
/// first widens to `float`).
fn boolean_conversion(s: &mut Sema<'_, '_>, e: ExprId, from: QualType, to: QualType) -> ExprId {
    let mut e = e;
    let mut from = from;
    if s.ast.types.builtin(from) == Some(BuiltinType::Half) {
        e = cast_to(s, CastKind::FloatingCast, e, QualType::FLOAT, None);
        from = QualType::FLOAT;
    }
    let ck = if s.ast.types.is_pointer(from)
        || s.ast.types.builtin(from) == Some(BuiltinType::NullPtr)
    {
        CastKind::PointerToBoolean
    } else if member_pointer_parts(s, from).is_some() {
        CastKind::MemberPointerToBoolean
    } else if let Some(el) = s.ast.types.as_complex(from) {
        if s.ast.types.is_floating(el) {
            CastKind::FloatingComplexToBoolean
        } else {
            CastKind::IntegralComplexToBoolean
        }
    } else if s.ast.types.is_floating(from) {
        CastKind::FloatingToBoolean
    } else if is_integral_or_unscoped_enum(s, from) || s.is_vector_type(from) {
        // An integer ext vector to a `bool` ext vector (clang 20+
        // `IsVectorConversion`): lane-wise `!= 0`.
        CastKind::IntegralToBoolean
    } else {
        CastKind::NoOp
    };
    cast_to(s, ck, e, to, None)
}

/// [conv.ptr]: a null pointer constant, a derived-to-base pointer
/// conversion (access-checked; ambiguity is an error) or a bit cast.
fn pointer_conversion(
    s: &mut Sema<'_, '_>,
    e: ExprId,
    scs: &StandardConversion,
    from: QualType,
    to: QualType,
) -> Result<ExprId, ConvError> {
    if scs.null_pointer || s.ast.types.builtin(from) == Some(BuiltinType::NullPtr) {
        return Ok(cast_to(s, CastKind::NullToPointer, e, to, None));
    }
    let (fp, tp) = (s.ast.types.pointee(from), s.ast.types.pointee(to));
    let classes = match (fp, tp) {
        (Some(fp), Some(tp)) => match (s.ast.types.as_record(fp), s.ast.types.as_record(tp)) {
            (Some(d), Some(b)) if !s.ast.types.same_unqualified(fp, tp) => Some((d, b, fp, tp)),
            _ => None,
        },
        _ => None,
    };
    let Some((d, b, fp, tp)) = classes else {
        return Ok(cast_to(s, CastKind::BitCast, e, to, None));
    };
    Ok(match find_base(s, d, b) {
        BaseSearch::Unique(p) => {
            // [conv.ptr]/3: the base must be accessible (diagnosed; the
            // conversion still applies).
            let at = s.begin_loc(e);
            s.check_base_access(strip_cv(s, fp), strip_cv(s, tp), &p, at);
            let list = path_list(s, &p);
            cast_to(s, CastKind::DerivedToBase, e, to, Some(list))
        }
        BaseSearch::Ambiguous(paths) => {
            let shown = ambiguous_paths_string(s, fp, &paths);
            let (fa, ta) = (s.ty_arg(strip_cv(s, fp)), s.ty_arg(strip_cv(s, tp)));
            return Err(ConvError::new(
                DiagId::sema_ambiguous_derived_to_base,
                vec![fa, ta, shown],
            ));
        }
        BaseSearch::NotDerived => cast_to(s, CastKind::BitCast, e, to, None),
    })
}

/// A class glvalue converted to its base: [over.best.ics]/6 ranks a class
/// prvalue converted to its base only as a conversion; initialization
/// performs it through a constructor, so only a glvalue view reaches here.
fn derived_to_base(
    s: &mut Sema<'_, '_>,
    e: ExprId,
    from: QualType,
    to: QualType,
) -> Result<ExprId, ConvError> {
    let (Some(d), Some(b)) = (s.ast.types.as_record(from), s.ast.types.as_record(to)) else {
        return Ok(e);
    };
    Ok(match find_base(s, d, b) {
        BaseSearch::Unique(p) => {
            let at = s.begin_loc(e);
            s.check_base_access(strip_cv(s, from), strip_cv(s, to), &p, at);
            let list = path_list(s, &p);
            let vk = s.ast.exprs.get(e).value_category();
            super::raw_cast(s, CastKind::DerivedToBase, e, to, vk, Some(list))
        }
        BaseSearch::Ambiguous(paths) => {
            let shown = ambiguous_paths_string(s, from, &paths);
            let (fa, ta) = (s.ty_arg(strip_cv(s, from)), s.ty_arg(strip_cv(s, to)));
            return Err(ConvError::new(
                DiagId::sema_ambiguous_derived_to_base,
                vec![fa, ta, shown],
            ));
        }
        BaseSearch::NotDerived => e,
    })
}

/// [conv.mem]: a null member pointer, or a base-to-derived member pointer
/// conversion (not through a virtual base; ambiguity is an error).
fn member_pointer_conversion(
    s: &mut Sema<'_, '_>,
    e: ExprId,
    scs: &StandardConversion,
    from: QualType,
    to: QualType,
) -> Result<ExprId, ConvError> {
    if scs.null_pointer || s.ast.types.builtin(from) == Some(BuiltinType::NullPtr) {
        return Ok(cast_to(s, CastKind::NullToMemberPointer, e, to, None));
    }
    let (Some((_, fc)), Some((_, tc))) =
        (member_pointer_parts(s, from), member_pointer_parts(s, to))
    else {
        return Ok(e);
    };
    let (Some(b), Some(d)) = (class_decl(s, fc), class_decl(s, tc)) else { return Ok(e) };
    let (fq, tq) = (QualType::unqual(fc), QualType::unqual(tc));
    Ok(match find_base(s, d, b) {
        BaseSearch::Unique(p) => {
            if let Some(v) = p.first_virtual {
                let vt = QualType::unqual(s.ast.types.record_type(v));
                return Err(ConvError::new(
                    DiagId::sema_member_pointer_conv_via_virtual,
                    vec![s.ty_arg(fq), s.ty_arg(tq), s.ty_arg(vt)],
                ));
            }
            let list = path_list(s, &p);
            cast_to(s, CastKind::BaseToDerivedMemberPointer, e, to, Some(list))
        }
        BaseSearch::Ambiguous(paths) => {
            let shown = ambiguous_paths_string(s, tq, &paths);
            return Err(ConvError::new(
                DiagId::sema_ambiguous_member_pointer_conv,
                vec!["0".into(), s.ty_arg(fq), s.ty_arg(tq), shown],
            ));
        }
        BaseSearch::NotDerived => e,
    })
}

/// The cast between two complex types, by their element kinds (clang
/// `ICK_Complex_Promotion` / `ICK_Complex_Conversion`).
fn complex_cast_kind(s: &Sema<'_, '_>, from: QualType, to: QualType) -> CastKind {
    let fl = |t: QualType| s.ast.types.as_complex(t).is_some_and(|el| s.ast.types.is_floating(el));
    match (fl(from), fl(to)) {
        (true, true) => CastKind::FloatingComplexCast,
        (true, false) => CastKind::FloatingComplexToIntegralComplex,
        (false, true) => CastKind::IntegralComplexToFloatingComplex,
        (false, false) => CastKind::IntegralComplexCast,
    }
}

/// The cast between two real arithmetic types (an element step of a
/// complex-real conversion).
fn real_cast_kind(s: &Sema<'_, '_>, from: QualType, to: QualType) -> Option<CastKind> {
    let types = &s.ast.types;
    if types.same_unqualified(from, to) {
        return None;
    }
    let to_bool = types.builtin(to) == Some(BuiltinType::Bool);
    Some(match (types.is_floating(from), types.is_floating(to)) {
        (true, true) => CastKind::FloatingCast,
        (true, false) if to_bool => CastKind::FloatingToBoolean,
        (true, false) => CastKind::FloatingToIntegral,
        (false, true) => CastKind::IntegralToFloating,
        (false, false) if to_bool => CastKind::IntegralToBoolean,
        (false, false) => CastKind::IntegralCast,
    })
}

/// clang's `ICK_Complex_Real`: a real value converts to its element type
/// and then to the complex type; a complex value takes its real part
/// (only under an explicit cast in C++) and then converts.
fn complex_real(
    s: &mut Sema<'_, '_>,
    e: ExprId,
    from: QualType,
    to: QualType,
) -> Result<ExprId, ConvError> {
    if let Some(el) = s.ast.types.as_complex(to) {
        let fl = s.ast.types.is_floating(el);
        let mut e = e;
        if let Some(ck) = real_cast_kind(s, from, el) {
            e = cast_to(s, ck, e, el, None);
        }
        let ck = if fl { CastKind::FloatingRealToComplex } else { CastKind::IntegralRealToComplex };
        return Ok(cast_to(s, ck, e, to, None));
    }
    let Some(el) = s.ast.types.as_complex(from) else { return Ok(e) };
    // C17 6.3.1.7p2: in C a complex value converts to a real type by
    // discarding its imaginary part.
    if !s.explicit_conv && !s.is_c() {
        let (fa, ta) = (s.ty_arg(from), s.ty_arg(to));
        let mut err = ConvError::new(DiagId::sema_complex_to_real_cxx, vec![fa, ta]);
        err.at = Some(s.begin_loc(e));
        return Err(err);
    }
    let ck = if s.ast.types.is_floating(el) {
        CastKind::FloatingComplexToReal
    } else {
        CastKind::IntegralComplexToReal
    };
    let e = cast_to(s, ck, e, el, None);
    Ok(match real_cast_kind(s, el, to) {
        Some(ck) => cast_to(s, ck, e, to, None),
        None => e,
    })
}

/// GCC's pedantic warning for an implicit floating-point conversion that
/// P1467 does not allow ([conv.double]/1; `fp_rank_violation`): kiln, as
/// GCC 14, accepts it with `sema_fp_conversion_rank`.
pub(crate) fn fp_rank_check(s: &mut Sema<'_, '_>, e: ExprId, from: QualType, to: QualType) {
    let (Some(f), Some(t)) = (s.ast.types.builtin(from), s.ast.types.builtin(to)) else {
        return;
    };
    if let Some(unordered) = super::rank::fp_rank_violation(s, f, t) {
        let loc = s.eloc(e);
        // GCC names both types without their cv-qualifiers.
        let (to, from) = (s.ast.types.unqualified(to), s.ast.types.unqualified(from));
        let msg = s
            .diag(DiagId::sema_fp_conversion_rank, loc)
            .arg(s.ty_arg(to))
            .arg(s.ty_arg(from))
            .arg(if unordered { "1" } else { "0" });
        s.emit(msg);
    }
}
