//! Standard conversion sequences ([over.ics.scs]) and their ranking
//! ([over.ics.rank]/3.2, /4), following clang's `IsStandardConversion`
//! and `CompareStandardConversionSequences`.

use kiln_ast::expr::ValueCategory;
use kiln_ast::types::{BuiltinType, FloatFormat, TypeView};
use kiln_ast::{ExprId, QualType, TypeId};

use super::bases::{find_base, is_derived_from};
use super::qual::{QualRelation, is_function_pointer_conversion, qualification_relation};
use super::rank::{
    FloatOrder, enum_data, float_format, float_order, format_subset, is_integral_or_unscoped_enum,
    is_scoped_enum, is_unscoped_enum, promoted_type,
};
use super::{ConvCtx, array_decayed, bit_field_width, is_null_pointer_constant, strip_cv};
use crate::Sema;

/// The first conversion of a standard conversion sequence ([conv.lval],
/// [conv.array], [conv.func]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FirstStep {
    /// None.
    Identity,
    /// Lvalue-to-rvalue conversion.
    LValueToRValue,
    /// Array-to-pointer conversion.
    ArrayToPointer,
    /// Function-to-pointer conversion.
    FunctionToPointer,
}

/// The second conversion: a promotion or conversion ([over.ics.scs]
/// table 19).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SecondStep {
    /// None.
    Identity,
    /// [conv.prom].
    IntegralPromotion,
    /// [conv.fpprom].
    FloatingPromotion,
    /// [conv.integral].
    IntegralConversion,
    /// [conv.double].
    FloatingConversion,
    /// [conv.fpint].
    FloatingIntegral,
    /// [conv.ptr], including null pointer conversions.
    PointerConversion,
    /// [conv.mem], including null member pointer conversions.
    MemberPointerConversion,
    /// [conv.bool].
    BooleanConversion,
}

/// The third conversion ([conv.fctptr], [conv.qual]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ThirdStep {
    /// None.
    Identity,
    /// Function pointer conversion.
    FunctionPointer,
    /// Qualification conversion.
    Qualification,
}

/// The rank of a standard conversion sequence ([over.ics.scs]/3),
/// ordered best first.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum ConvRank {
    /// Exact Match.
    ExactMatch,
    /// Promotion.
    Promotion,
    /// Conversion.
    Conversion,
}

/// How an unscoped enumeration with a fixed underlying type was promoted
/// ([over.ics.rank]/4.2).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FixedEnumPromotion {
    /// Not such a promotion.
    None,
    /// To the underlying type.
    ToUnderlying,
    /// To the promoted underlying type.
    ToPromotedUnderlying,
}

/// A standard conversion sequence ([over.ics.scs]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StandardConversion {
    /// The first conversion.
    pub first: FirstStep,
    /// The second conversion.
    pub second: SecondStep,
    /// The third conversion.
    pub third: ThirdStep,
    /// The source expression's type.
    pub from: QualType,
    /// The type after each conversion (clang's `getToType(0..3)`).
    pub to_types: [QualType; 3],
    /// The source is a null pointer constant and the second conversion a
    /// null (member) pointer conversion.
    pub null_pointer: bool,
    /// See [`FixedEnumPromotion`].
    pub fixed_enum: FixedEnumPromotion,
}

impl StandardConversion {
    /// The rank ([over.ics.scs]/3): that of the second conversion.
    pub fn rank(&self) -> ConvRank {
        match self.second {
            SecondStep::Identity => ConvRank::ExactMatch,
            SecondStep::IntegralPromotion | SecondStep::FloatingPromotion => ConvRank::Promotion,
            _ => ConvRank::Conversion,
        }
    }

    /// True if no conversion other than an lvalue transformation is made.
    pub fn is_identity(&self) -> bool {
        self.second == SecondStep::Identity && self.third == ThirdStep::Identity
    }
}

/// The class of a canonical member pointer type: `(pointee, class)`.
pub(crate) fn member_pointer_parts(s: &Sema<'_, '_>, q: QualType) -> Option<(QualType, TypeId)> {
    match s.ast.types.canonical_view(q) {
        TypeView::MemberPointer { pointee, class } => Some((pointee, class)),
        _ => None,
    }
}

pub(crate) fn class_decl(s: &Sema<'_, '_>, t: TypeId) -> Option<kiln_ast::DeclId> {
    s.ast.types.as_record(QualType::unqual(t))
}

fn is_nullptr_t(s: &Sema<'_, '_>, q: QualType) -> bool {
    s.ast.types.builtin(q) == Some(BuiltinType::NullPtr)
}

fn is_member_pointer(s: &Sema<'_, '_>, q: QualType) -> bool {
    member_pointer_parts(s, q).is_some()
}

/// clang's `BuildSimilarlyQualifiedPointerType`: a pointer to `to`'s
/// pointee carrying `from`'s pointee qualifiers.
fn similarly_qualified_pointer(
    s: &mut Sema<'_, '_>,
    from_pointee: QualType,
    to: QualType,
) -> QualType {
    let to_pointee = s.ast.types.pointee(to).unwrap_or(QualType::ERROR);
    let cf = s.ast.types.canonical(from_pointee);
    let ct = s.ast.types.canonical(to_pointee);
    if cf.quals() == ct.quals() {
        return strip_cv(s, to);
    }
    let p = s.ast.types.qualify(ct.without_quals(), cf.quals());
    s.ast.types.pointer_to(p)
}

fn integral_promotion_kind(
    s: &Sema<'_, '_>,
    from: QualType,
    to: QualType,
    bit_field: Option<u32>,
) -> Option<FixedEnumPromotion> {
    // The target is a (non-enumeration) integral type.
    let TypeView::Builtin(tb) = s.ast.types.canonical_view(to) else { return None };
    if !tb.is_integral() {
        return None;
    }
    let to_c = s.ast.types.canonical(to).without_quals();
    let same = |q: QualType| s.ast.types.canonical(q).without_quals() == to_c;
    if s.ast.types.as_enum(from).is_some() {
        if is_scoped_enum(s, from) {
            return None;
        }
        let e = enum_data(s, from)?;
        if e.is_fixed() {
            let u = e.int_type?;
            if same(u) {
                return Some(FixedEnumPromotion::ToUnderlying);
            }
            let p = promoted_type(s, u, None)?;
            return same(p).then_some(FixedEnumPromotion::ToPromotedUnderlying);
        }
        let p = e.promotion_type?;
        return same(p).then_some(FixedEnumPromotion::None);
    }
    let p = promoted_type(s, from, bit_field)?;
    same(p).then_some(FixedEnumPromotion::None)
}

fn is_floating_promotion(s: &Sema<'_, '_>, from: QualType, to: QualType) -> bool {
    matches!(
        (s.ast.types.builtin(from), s.ast.types.builtin(to)),
        (Some(BuiltinType::Float), Some(BuiltinType::Double))
            | (Some(BuiltinType::Half), Some(BuiltinType::Float))
    )
}

fn is_floating_conversion(s: &Sema<'_, '_>, from: QualType, to: QualType) -> bool {
    let (Some(f), Some(t)) = (s.ast.types.builtin(from), s.ast.types.builtin(to)) else {
        return false;
    };
    if !f.is_floating() || !t.is_floating() {
        return false;
    }
    let half16 = |b: BuiltinType| matches!(b, BuiltinType::Float16 | BuiltinType::Half);
    if (f == BuiltinType::BFloat16 && half16(t)) || (t == BuiltinType::BFloat16 && half16(f)) {
        return false;
    }
    let (ff, tf) = (float_format(s, f), float_format(s, t));
    !matches!(
        (ff, tf),
        (Some(FloatFormat::PpcDoubleDouble), Some(FloatFormat::IeeeQuad))
            | (Some(FloatFormat::IeeeQuad), Some(FloatFormat::PpcDoubleDouble))
    )
}

/// The standard conversion sequence converting `e` to `to` in `ctx`
/// ([over.ics.scs]), or `None` if there is none. Class types never
/// have one.
pub fn standard_conversion(
    s: &mut Sema<'_, '_>,
    e: ExprId,
    to: QualType,
    ctx: ConvCtx,
) -> Option<StandardConversion> {
    let n = *s.ast.exprs.get(e);
    let bw = bit_field_width(s, e);
    let npc = is_null_pointer_constant(s, e);
    standard_conversion_from(s, n.ty(), n.value_category(), npc, bw, to, ctx)
}

/// [`standard_conversion`] from an expression described by its type,
/// value category, whether it is a null pointer constant, and the width
/// of the bit-field it designates.
pub fn standard_conversion_from(
    s: &mut Sema<'_, '_>,
    from: QualType,
    vk: ValueCategory,
    npc: bool,
    bit_field: Option<u32>,
    to: QualType,
    ctx: ConvCtx,
) -> Option<StandardConversion> {
    let types = &s.ast.types;
    if types.as_record(from).is_some() || types.as_record(to).is_some() {
        return None;
    }
    if types.is_reference(to) || types.is_array(to) || types.is_function(to) {
        return None;
    }
    // First step.
    let (first, cur) = if types.is_function(from) {
        if vk != ValueCategory::LValue {
            return None;
        }
        (FirstStep::FunctionToPointer, s.ast.types.pointer_to(from))
    } else if types.is_array(from) {
        (FirstStep::ArrayToPointer, array_decayed(s, from))
    } else if vk.is_glvalue() && !types.is_void(from) {
        (FirstStep::LValueToRValue, strip_cv(s, from))
    } else {
        (FirstStep::Identity, from)
    };
    let mut scs = StandardConversion {
        first,
        second: SecondStep::Identity,
        third: ThirdStep::Identity,
        from,
        to_types: [cur, cur, cur],
        null_pointer: false,
        fixed_enum: FixedEnumPromotion::None,
    };
    // A bit-field width only matters before the lvalue transformation
    // drops it; it stays attached to the value.
    let bw = if first == FirstStep::LValueToRValue || first == FirstStep::Identity {
        bit_field
    } else {
        None
    };
    let types = &s.ast.types;
    let (second, conv) = if types.same_unqualified(cur, to) {
        (SecondStep::Identity, cur)
    } else if let Some(fe) = integral_promotion_kind(s, cur, to, bw) {
        scs.fixed_enum = fe;
        (SecondStep::IntegralPromotion, to)
    } else if is_floating_promotion(s, cur, to) {
        (SecondStep::FloatingPromotion, to)
    } else if types.builtin(to) == Some(BuiltinType::Bool)
        && (super::rank::is_arithmetic_or_unscoped_enum(s, cur)
            || types.is_pointer(cur)
            || is_member_pointer(s, cur)
            || (is_nullptr_t(s, cur) && ctx.is_direct()))
    {
        (SecondStep::BooleanConversion, to)
    } else if is_integral_or_unscoped_enum(s, cur) && types.is_integral(to) {
        (SecondStep::IntegralConversion, to)
    } else if is_floating_conversion(s, cur, to) {
        (SecondStep::FloatingConversion, to)
    } else if (types.is_floating(cur) && types.is_integral(to))
        || (is_integral_or_unscoped_enum(s, cur) && types.is_floating(to))
    {
        (SecondStep::FloatingIntegral, to)
    } else if let Some(t) = pointer_conversion(s, cur, to, npc) {
        scs.null_pointer = npc && !types_pointer_like(s, cur);
        (SecondStep::PointerConversion, t)
    } else if let Some(t) = member_pointer_conversion(s, cur, to, npc) {
        scs.null_pointer = npc && !is_member_pointer(s, cur);
        (SecondStep::MemberPointerConversion, t)
    } else {
        (SecondStep::Identity, cur)
    };
    scs.second = second;
    scs.to_types[1] = conv;
    // Third step.
    let third = if is_function_pointer_conversion(s, conv, to) {
        ThirdStep::FunctionPointer
    } else {
        match qualification_relation(s, conv, to) {
            QualRelation::Similar { identical: true, .. } => ThirdStep::Identity,
            QualRelation::Similar { convertible: true, .. } => ThirdStep::Qualification,
            _ => {
                if s.ast.types.same_unqualified(conv, to) {
                    ThirdStep::Identity
                } else {
                    return None;
                }
            }
        }
    };
    if third == ThirdStep::Identity && !s.ast.types.same_unqualified(conv, to) {
        return None;
    }
    scs.third = third;
    scs.to_types[2] = to;
    Some(scs)
}

fn types_pointer_like(s: &Sema<'_, '_>, q: QualType) -> bool {
    s.ast.types.is_pointer(q)
}

/// [conv.ptr]: the type after a pointer conversion of a `from` value
/// (`npc`: a null pointer constant) towards `to`.
fn pointer_conversion(
    s: &mut Sema<'_, '_>,
    from: QualType,
    to: QualType,
    npc: bool,
) -> Option<QualType> {
    if is_nullptr_t(s, to) {
        return npc.then_some(to);
    }
    let TypeView::Pointer(to_pointee) = s.ast.types.canonical_view(to) else { return None };
    if npc {
        return Some(to);
    }
    let TypeView::Pointer(from_pointee) = s.ast.types.canonical_view(from) else { return None };
    if s.ast.types.same_unqualified(from_pointee, to_pointee) {
        return None;
    }
    if s.ast.types.is_void(to_pointee) && !s.ast.types.is_function(from_pointee) {
        return Some(similarly_qualified_pointer(s, from_pointee, to));
    }
    if let (Some(d), Some(b)) =
        (s.ast.types.as_record(from_pointee), s.ast.types.as_record(to_pointee))
        && find_base(s, d, b).is_derived()
    {
        return Some(similarly_qualified_pointer(s, from_pointee, to));
    }
    None
}

/// [conv.mem]: the type after a pointer-to-member conversion.
fn member_pointer_conversion(
    s: &mut Sema<'_, '_>,
    from: QualType,
    to: QualType,
    npc: bool,
) -> Option<QualType> {
    let (_, to_class) = member_pointer_parts(s, to)?;
    if npc {
        return Some(to);
    }
    let (from_pointee, from_class) = member_pointer_parts(s, from)?;
    let (b, d) = (class_decl(s, from_class)?, class_decl(s, to_class)?);
    if s.ast.types.same_unqualified(QualType::unqual(from_class), QualType::unqual(to_class)) {
        return None;
    }
    if !is_derived_from(s, d, b) {
        return None;
    }
    Some(s.ast.types.member_pointer(from_pointee, to_class))
}

/// The result of comparing two conversion sequences ([over.ics.rank]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IcsCompare {
    /// The first is better.
    Better,
    /// Neither is better.
    Indistinguishable,
    /// The first is worse.
    Worse,
}

fn same(s: &Sema<'_, '_>, a: QualType, b: QualType) -> bool {
    s.ast.types.same_type(a, b)
}

/// [over.ics.rank]/3.2.1: a proper subsequence is better (clang's
/// `compareStandardConversionSubsets`; lvalue transformations are
/// excluded).
fn compare_subsets(s: &Sema<'_, '_>, a: &StandardConversion, b: &StandardConversion) -> IcsCompare {
    if a.is_identity() && !b.is_identity() {
        return IcsCompare::Better;
    }
    if b.is_identity() && !a.is_identity() {
        return IcsCompare::Worse;
    }
    let mut result = IcsCompare::Indistinguishable;
    if a.second != b.second {
        if a.second == SecondStep::Identity {
            result = IcsCompare::Better;
        } else if b.second == SecondStep::Identity {
            result = IcsCompare::Worse;
        } else {
            return IcsCompare::Indistinguishable;
        }
    } else if !super::qual::similar(s, a.to_types[1], b.to_types[1]) {
        return IcsCompare::Indistinguishable;
    }
    if a.third == b.third {
        return if same(s, a.to_types[2], b.to_types[2]) {
            result
        } else {
            IcsCompare::Indistinguishable
        };
    }
    if a.third == ThirdStep::Identity {
        return if result == IcsCompare::Worse {
            IcsCompare::Indistinguishable
        } else {
            IcsCompare::Better
        };
    }
    if b.third == ThirdStep::Identity {
        return if result == IcsCompare::Better {
            IcsCompare::Indistinguishable
        } else {
            IcsCompare::Worse
        };
    }
    IcsCompare::Indistinguishable
}

/// [over.ics.rank]/4.1: a conversion of a pointer, pointer to member or
/// `std::nullptr_t` to `bool`.
fn is_pointer_to_bool(s: &Sema<'_, '_>, c: &StandardConversion) -> bool {
    if s.ast.types.builtin(c.to_types[1]) != Some(BuiltinType::Bool) {
        return false;
    }
    let f = c.to_types[0];
    s.ast.types.is_pointer(f)
        || is_member_pointer(s, f)
        || is_nullptr_t(s, f)
        || matches!(c.first, FirstStep::ArrayToPointer | FirstStep::FunctionToPointer)
}

/// A pointer conversion of a pointer to non-void to a pointer to `void`.
fn is_pointer_to_void(s: &Sema<'_, '_>, c: &StandardConversion) -> bool {
    if c.second != SecondStep::PointerConversion || c.null_pointer {
        return false;
    }
    let (Some(fp), Some(tp)) =
        (s.ast.types.pointee(c.to_types[0]), s.ast.types.pointee(c.to_types[1]))
    else {
        return false;
    };
    s.ast.types.is_pointer(c.to_types[0]) && s.ast.types.is_void(tp) && !s.ast.types.is_void(fp)
}

/// True if class type `d` is a proper derived class of class type `b`.
fn derived_q(s: &Sema<'_, '_>, d: QualType, b: QualType) -> bool {
    if s.ast.types.same_unqualified(d, b) {
        return false;
    }
    match (s.ast.types.as_record(d), s.ast.types.as_record(b)) {
        (Some(d), Some(b)) => is_derived_from(s, d, b),
        _ => false,
    }
}

/// [over.ics.rank]/4.4: conversions between pointers to classes and
/// pointers to members ordered by derivation (clang's
/// `CompareDerivedToBaseConversions`).
fn compare_derived_to_base(
    s: &Sema<'_, '_>,
    a: &StandardConversion,
    b: &StandardConversion,
) -> IcsCompare {
    let (f1, t1, f2, t2) = (a.to_types[0], a.to_types[1], b.to_types[0], b.to_types[1]);
    let t = &s.ast.types;
    if a.second == SecondStep::PointerConversion
        && b.second == SecondStep::PointerConversion
        && t.is_pointer(f1)
        && t.is_pointer(t1)
        && t.is_pointer(f2)
        && t.is_pointer(t2)
    {
        let p = |q: QualType| t.canonical(t.pointee(q).unwrap_or(QualType::ERROR)).without_quals();
        let (fp1, tp1, fp2, tp2) = (p(f1), p(t1), p(f2), p(t2));
        if t.as_record(fp1).is_some()
            && t.as_record(tp1).is_some()
            && t.as_record(fp2).is_some()
            && t.as_record(tp2).is_some()
        {
            if fp1 == fp2 && tp1 != tp2 {
                // A -> B is better than A -> C if B derives from C.
                if derived_q(s, tp1, tp2) {
                    return IcsCompare::Better;
                }
                if derived_q(s, tp2, tp1) {
                    return IcsCompare::Worse;
                }
            }
            if tp1 == tp2 && fp1 != fp2 {
                // B -> A is better than C -> A if C derives from B.
                if derived_q(s, fp2, fp1) {
                    return IcsCompare::Better;
                }
                if derived_q(s, fp1, fp2) {
                    return IcsCompare::Worse;
                }
            }
        }
    }
    if a.second == SecondStep::MemberPointerConversion
        && b.second == SecondStep::MemberPointerConversion
        && !a.null_pointer
        && !b.null_pointer
    {
        let c = |q: QualType| {
            member_pointer_parts(s, q)
                .map(|(_, c)| s.ast.types.canonical(QualType::unqual(c)).without_quals())
        };
        if let (Some(fc1), Some(tc1), Some(fc2), Some(tc2)) = (c(f1), c(t1), c(f2), c(t2)) {
            if fc1 == fc2 && tc1 != tc2 {
                // A::* -> B::* is better than A::* -> C::* if C derives
                // from B.
                if derived_q(s, tc1, tc2) {
                    return IcsCompare::Worse;
                }
                if derived_q(s, tc2, tc1) {
                    return IcsCompare::Better;
                }
            }
            if tc1 == tc2 && fc1 != fc2 {
                // B::* -> C::* is better than A::* -> C::* if B derives
                // from A.
                if derived_q(s, fc1, fc2) {
                    return IcsCompare::Better;
                }
                if derived_q(s, fc2, fc1) {
                    return IcsCompare::Worse;
                }
            }
        }
    }
    IcsCompare::Indistinguishable
}

fn float_builtin(s: &Sema<'_, '_>, q: QualType) -> Option<BuiltinType> {
    s.ast.types.builtin(q).filter(|b| b.is_floating())
}

fn same_fp_rank(s: &Sema<'_, '_>, x: BuiltinType, y: BuiltinType) -> bool {
    match (float_format(s, x), float_format(s, y)) {
        (Some(p), Some(q)) => format_subset(p, q) && format_subset(q, p),
        _ => false,
    }
}

/// [over.ics.rank]/4.3: a conversion between `fp1` and `fp2` beats one
/// between `fp1` and `t3` when `fp2` has the rank of `fp1` and `t3` is
/// not a floating-point type, has another rank, or a lesser subrank
/// than `fp2`.
fn fp_better(s: &Sema<'_, '_>, fp1: BuiltinType, fp2: QualType, t3: QualType) -> bool {
    let Some(fp2) = float_builtin(s, fp2) else { return false };
    if !same_fp_rank(s, fp1, fp2) {
        return false;
    }
    match float_builtin(s, t3) {
        None => s.ast.types.is_arithmetic(t3) || is_unscoped_enum(s, t3),
        Some(t3) => !same_fp_rank(s, t3, fp1) || float_order(s, fp2, t3) == FloatOrder::Greater,
    }
}

/// [over.ics.rank]/4.3 (C++23, extended floating-point types), in both
/// directions: from one floating-point type, or to one.
fn compare_fp(s: &Sema<'_, '_>, a: &StandardConversion, b: &StandardConversion) -> IcsCompare {
    if a.second == SecondStep::Identity || b.second == SecondStep::Identity {
        return IcsCompare::Indistinguishable;
    }
    let c = |q: QualType| s.ast.types.canonical(q).without_quals();
    let (f1, t1, f2, t2) = (a.to_types[0], a.to_types[1], b.to_types[0], b.to_types[1]);
    if c(f1) == c(f2)
        && c(t1) != c(t2)
        && let Some(fp1) = float_builtin(s, f1)
    {
        if fp_better(s, fp1, t1, t2) {
            return IcsCompare::Better;
        }
        if fp_better(s, fp1, t2, t1) {
            return IcsCompare::Worse;
        }
    }
    if c(t1) == c(t2)
        && c(f1) != c(f2)
        && let Some(fp1) = float_builtin(s, t1)
    {
        if fp_better(s, fp1, f1, f2) {
            return IcsCompare::Better;
        }
        if fp_better(s, fp1, f2, f1) {
            return IcsCompare::Worse;
        }
    }
    IcsCompare::Indistinguishable
}

/// Compares two standard conversion sequences ([over.ics.rank]/3.2,
/// /4).
pub fn compare_standard_conversions(
    s: &Sema<'_, '_>,
    a: &StandardConversion,
    b: &StandardConversion,
) -> IcsCompare {
    // /3.2.1.
    let sub = compare_subsets(s, a, b);
    if sub != IcsCompare::Indistinguishable {
        return sub;
    }
    // /3.2.2.
    match a.rank().cmp(&b.rank()) {
        std::cmp::Ordering::Less => return IcsCompare::Better,
        std::cmp::Ordering::Greater => return IcsCompare::Worse,
        std::cmp::Ordering::Equal => {}
    }
    // /4.1: a conversion not converting a pointer to bool is better.
    match (is_pointer_to_bool(s, a), is_pointer_to_bool(s, b)) {
        (false, true) => return IcsCompare::Better,
        (true, false) => return IcsCompare::Worse,
        _ => {}
    }
    // /4.2: promotion of a fixed-underlying enum to its underlying type.
    match (a.fixed_enum, b.fixed_enum) {
        (FixedEnumPromotion::ToUnderlying, FixedEnumPromotion::ToPromotedUnderlying) => {
            return IcsCompare::Better;
        }
        (FixedEnumPromotion::ToPromotedUnderlying, FixedEnumPromotion::ToUnderlying) => {
            return IcsCompare::Worse;
        }
        _ => {}
    }
    // /4.4.1-2: conversions to void* are worse than other pointer
    // conversions; then derived-to-base orderings.
    let (v1, v2) = (is_pointer_to_void(s, a), is_pointer_to_void(s, b));
    if v1 != v2 {
        return if v2 { IcsCompare::Better } else { IcsCompare::Worse };
    }
    if v1 {
        // Both to void*: B* -> void* is better than A* -> void* if B
        // derives from A.
        let p = |q: QualType| {
            let t = &s.ast.types;
            t.canonical(t.pointee(q).unwrap_or(QualType::ERROR)).without_quals()
        };
        let (fp1, fp2) = (p(a.to_types[0]), p(b.to_types[0]));
        if fp1 != fp2 {
            if derived_q(s, fp1, fp2) {
                return IcsCompare::Better;
            }
            if derived_q(s, fp2, fp1) {
                return IcsCompare::Worse;
            }
        }
    } else {
        let d = compare_derived_to_base(s, a, b);
        if d != IcsCompare::Indistinguishable {
            return d;
        }
    }
    // /3.2.5: qualification conversions differing only in qualifiers.
    if a.first == b.first
        && a.second == b.second
        && a.third == ThirdStep::Qualification
        && b.third == ThirdStep::Qualification
    {
        let c = |q: QualType| s.ast.types.canonical(q).without_quals();
        let (t1, t2) = (c(a.to_types[2]), c(b.to_types[2]));
        if t1 != t2 && super::qual::similar(s, t1, t2) {
            let (x, y) = (
                super::qual::is_qualification_convertible(s, t1, t2),
                super::qual::is_qualification_convertible(s, t2, t1),
            );
            if x && !y {
                return IcsCompare::Better;
            }
            if y && !x {
                return IcsCompare::Worse;
            }
        }
    }
    // /4.3: floating-point conversions of equal rank.
    let f = compare_fp(s, a, b);
    if f != IcsCompare::Indistinguishable {
        return f;
    }
    IcsCompare::Indistinguishable
}
