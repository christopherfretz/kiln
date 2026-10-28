//! Performing a standard conversion sequence: the `ImplicitCastExpr`s
//! clang's `PerformImplicitConversion` builds for each step.

use kiln_ast::expr::CastKind;
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
    Ok(e)
}

#[allow(clippy::too_many_lines, reason = "one arm per second-step kind")]
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
        SecondStep::IntegralPromotion | SecondStep::IntegralConversion => {
            let ck = if to_bool { CastKind::IntegralToBoolean } else { CastKind::IntegralCast };
            cast_to(s, ck, e, to, None)
        }
        SecondStep::FloatingPromotion | SecondStep::FloatingConversion => {
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
        SecondStep::BooleanConversion => {
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
            } else if s.ast.types.is_floating(from) {
                CastKind::FloatingToBoolean
            } else if is_integral_or_unscoped_enum(s, from) {
                CastKind::IntegralToBoolean
            } else {
                CastKind::NoOp
            };
            cast_to(s, ck, e, to, None)
        }
        SecondStep::PointerConversion => {
            if scs.null_pointer || s.ast.types.builtin(from) == Some(BuiltinType::NullPtr) {
                return Ok(cast_to(s, CastKind::NullToPointer, e, to, None));
            }
            let (fp, tp) = (s.ast.types.pointee(from), s.ast.types.pointee(to));
            let classes = match (fp, tp) {
                (Some(fp), Some(tp)) => {
                    match (s.ast.types.as_record(fp), s.ast.types.as_record(tp)) {
                        (Some(d), Some(b)) if !s.ast.types.same_unqualified(fp, tp) => {
                            Some((d, b, fp, tp))
                        }
                        _ => None,
                    }
                }
                _ => None,
            };
            match classes {
                Some((d, b, fp, tp)) => match find_base(s, d, b) {
                    BaseSearch::Unique(p) => {
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
                },
                None => cast_to(s, CastKind::BitCast, e, to, None),
            }
        }
        SecondStep::MemberPointerConversion => {
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
            match find_base(s, d, b) {
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
            }
        }
    })
}
