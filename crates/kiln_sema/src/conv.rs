//! Standard conversions ([conv]) and the usual arithmetic conversions
//! ([expr.arith.conv]).
//!
//! STUB(WP-049): a placeholder with the agreed signatures; WP-049 owns
//! the real implementation and replaces this file wholesale. The stub
//! implements the conversions M2 programs need with clang's cast kinds
//! (decay, lvalue-to-rvalue, integral and floating conversions and
//! promotions, boolean conversions, null pointer and qualification
//! conversions, `T*` to `void*`) and reports every other conversion with
//! a placeholder diagnostic.

use kiln_ast::expr::{CastKind, ExprKind, ValueCategory};
use kiln_ast::types::BuiltinType;
use kiln_ast::{ExprId, QualType};
use kiln_diag::DiagId;

use crate::Sema;

/// Where a conversion happens (selects clang's wording).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConvCtx {
    /// Copy-initialization of a variable or member.
    Initialization,
    /// The right operand of `=`.
    Assignment,
    /// A function argument.
    Argument,
    /// A `return` operand.
    Return,
    /// An operand of a comparison.
    Comparison,
    /// A condition.
    Condition,
    /// The implicit part of an explicit cast.
    Cast,
}

/// A conversion that cannot be made (`id` is emitted at the expression).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ConvError {
    /// The diagnostic.
    pub id: DiagId,
    /// Its arguments.
    pub args: Vec<String>,
}

fn cast(s: &mut Sema<'_, '_>, ck: CastKind, e: ExprId, ty: QualType) -> ExprId {
    s.ast.builder().implicit_cast(ck, e, ty, ValueCategory::PRValue)
}

fn ty(s: &Sema<'_, '_>, e: ExprId) -> QualType {
    s.ast.exprs.get(e).ty()
}

fn is_record(s: &Sema<'_, '_>, q: QualType) -> bool {
    s.ast.types.as_record(q).is_some()
}

/// The lvalue-to-rvalue conversion [conv.lval] (identity on prvalues and
/// class types).
pub fn lvalue_to_rvalue(s: &mut Sema<'_, '_>, e: ExprId) -> ExprId {
    let n = *s.ast.exprs.get(e);
    if n.value_category() == ValueCategory::PRValue || is_record(s, n.ty()) {
        return e;
    }
    if s.ast.types.is_void(n.ty()) {
        return e;
    }
    let to = n.ty().without_quals();
    let to = unqualified_sugar(s, to);
    cast(s, CastKind::LValueToRValue, e, to)
}

/// `q` without cv-qualifiers (through sugar).
fn unqualified_sugar(s: &Sema<'_, '_>, q: QualType) -> QualType {
    if s.ast.types.effective_quals(q).is_empty() {
        q
    } else {
        s.ast.types.canonical(q).without_quals()
    }
}

/// The default conversions: array-to-pointer, function-to-pointer and
/// lvalue-to-rvalue [conv.array], [conv.func], [conv.lval].
pub fn decay(s: &mut Sema<'_, '_>, e: ExprId) -> ExprId {
    let q = ty(s, e);
    if let Some(elem) = s.ast.types.element(q) {
        if s.ast.types.is_array(q) {
            let p = s.ast.types.pointer_to(elem);
            return cast(s, CastKind::ArrayToPointerDecay, e, p);
        }
    }
    if s.ast.types.is_function(q) {
        let p = s.ast.types.pointer_to(q);
        return cast(s, CastKind::FunctionToPointerDecay, e, p);
    }
    lvalue_to_rvalue(s, e)
}

/// Integer conversion rank order ([conv.rank]); `None` for non-integers.
fn rank(b: BuiltinType) -> Option<u8> {
    Some(match b {
        BuiltinType::Bool => 0,
        BuiltinType::Char | BuiltinType::SChar | BuiltinType::UChar | BuiltinType::Char8 => 1,
        BuiltinType::Short | BuiltinType::UShort | BuiltinType::Char16 => 2,
        BuiltinType::Int | BuiltinType::UInt | BuiltinType::WChar | BuiltinType::Char32 => 3,
        BuiltinType::Long | BuiltinType::ULong => 4,
        BuiltinType::LongLong | BuiltinType::ULongLong => 5,
        BuiltinType::Int128 | BuiltinType::UInt128 => 6,
        _ => return None,
    })
}

fn signed(s: &Sema<'_, '_>, b: BuiltinType) -> bool {
    match b {
        BuiltinType::Char => s.target.char_signed,
        BuiltinType::WChar => s.target.wchar_signed,
        _ => b.is_signed_integer(),
    }
}

fn width(s: &Sema<'_, '_>, b: BuiltinType) -> u32 {
    u32::from(s.target.builtin_size_align(b).0) * 8
}

/// The promoted type of `q`, or `q` if it does not promote.
fn promoted_type(s: &Sema<'_, '_>, q: QualType, bit_width: Option<u32>) -> QualType {
    let t = &s.ast.types;
    if let Some(d) = t.as_enum(q) {
        let def = s.ast.definition(d).unwrap_or(d);
        if let Some(data) = s.ast.decls.enumeration(def) {
            if data.is_scoped() {
                return q;
            }
            if data.is_fixed() {
                return match data.int_type {
                    Some(u) => promoted_type(s, u, None),
                    None => q,
                };
            }
            if let Some(p) = data.promotion_type {
                return p;
            }
        }
        return q;
    }
    let Some(b) = t.builtin(q) else { return q };
    if let Some(w) = bit_width {
        if b.is_integral() && b != BuiltinType::Bool {
            if w < 32 || (w == 32 && signed(s, b)) {
                return QualType::INT;
            }
            if w == 32 {
                return QualType::UINT;
            }
        }
    }
    match b {
        BuiltinType::Bool
        | BuiltinType::Char
        | BuiltinType::SChar
        | BuiltinType::UChar
        | BuiltinType::Char8
        | BuiltinType::Short
        | BuiltinType::UShort
        | BuiltinType::Char16 => QualType::INT,
        BuiltinType::WChar | BuiltinType::Char32 => {
            if signed(s, b) { QualType::INT } else { QualType::UINT }
        }
        _ => q,
    }
}

/// The bit width of `e` if it designates a bit-field.
fn bitfield_width(s: &Sema<'_, '_>, mut e: ExprId) -> Option<u32> {
    loop {
        let n = s.ast.exprs.get(e);
        match n.kind() {
            ExprKind::ParenExpr => e = s.ast.exprs.operand(e),
            ExprKind::ImplicitCastExpr if s.ast.exprs.cast_kind(e) == CastKind::LValueToRValue => {
                e = s.ast.exprs.operand(e);
            }
            ExprKind::MemberExpr => {
                let f = s.ast.exprs.decl(e);
                let w = s.ast.decls.field(f)?.bit_width?;
                let v = crate::constfold::fold_int_expr(&s.ast, w)?;
                return u32::try_from(v).ok();
            }
            _ => return None,
        }
    }
}

/// Integral promotion [conv.prom] of `e` (after the default conversions).
pub fn integral_promotion(s: &mut Sema<'_, '_>, e: ExprId) -> ExprId {
    let bw = bitfield_width(s, e);
    let e = decay(s, e);
    let q = ty(s, e);
    let p = promoted_type(s, q, bw);
    if s.ast.types.same_unqualified(p, q) {
        return e;
    }
    cast(s, CastKind::IntegralCast, e, p)
}

fn incompatible(s: &Sema<'_, '_>, from: QualType, to: QualType) -> ConvError {
    ConvError { id: DiagId::sema_stub_incompatible, args: vec![s.ty_arg(to), s.ty_arg(from)] }
}

fn is_null_constant(s: &Sema<'_, '_>, e: ExprId) -> bool {
    let n = s.ast.exprs.get(e);
    match n.kind() {
        ExprKind::CXXNullPtrLiteralExpr => true,
        ExprKind::IntegerLiteral => s.ast.exprs.int_value(e) == Some(0),
        ExprKind::ParenExpr => is_null_constant(s, s.ast.exprs.operand(e)),
        _ => s.ast.types.builtin(n.ty()) == Some(BuiltinType::NullPtr),
    }
}

/// The scalar conversion from `e`'s (decayed, prvalue) type to `to`.
fn scalar_kind(s: &Sema<'_, '_>, e: ExprId, from: QualType, to: QualType) -> Option<CastKind> {
    let t = &s.ast.types;
    let to_bool = t.builtin(to) == Some(BuiltinType::Bool);
    let from_int = t.is_integral(from) || unscoped_enum(s, from);
    let from_enum_scoped = t.as_enum(from).is_some() && !unscoped_enum(s, from);
    if from_enum_scoped {
        return None;
    }
    if t.as_enum(to).is_some() {
        return None;
    }
    if t.is_floating(from) {
        if to_bool {
            return Some(CastKind::FloatingToBoolean);
        }
        if t.is_integral(to) {
            return Some(CastKind::FloatingToIntegral);
        }
        if t.is_floating(to) {
            return Some(CastKind::FloatingCast);
        }
        return None;
    }
    if from_int {
        if to_bool {
            return Some(CastKind::IntegralToBoolean);
        }
        if t.is_integral(to) {
            return Some(CastKind::IntegralCast);
        }
        if t.is_floating(to) {
            return Some(CastKind::IntegralToFloating);
        }
        if t.is_pointer(to) && is_null_constant(s, e) {
            return Some(CastKind::NullToPointer);
        }
        return None;
    }
    if t.builtin(from) == Some(BuiltinType::NullPtr) {
        if t.is_pointer(to) {
            return Some(CastKind::NullToPointer);
        }
        // `nullptr` to bool is direct-initialization only.
        return None;
    }
    if t.is_pointer(from) {
        if to_bool {
            return Some(CastKind::PointerToBoolean);
        }
        if t.is_pointer(to) {
            let (fp, tp) = (t.pointee(from)?, t.pointee(to)?);
            let (fq, tq) = (t.effective_quals(fp), t.effective_quals(tp));
            if !tq.contains(fq) {
                return None;
            }
            if t.same_unqualified(fp, tp) {
                return Some(CastKind::NoOp);
            }
            if t.is_void(tp) {
                return Some(CastKind::BitCast);
            }
        }
    }
    None
}

fn unscoped_enum(s: &Sema<'_, '_>, q: QualType) -> bool {
    s.ast.types.as_enum(q).is_some_and(|d| {
        let def = s.ast.definition(d).unwrap_or(d);
        s.ast.decls.enumeration(def).is_some_and(|e| !e.is_scoped())
    })
}

/// Implicitly converts `e` to `to` [conv], [dcl.init].
///
/// # Errors
/// If no implicit conversion sequence exists.
pub fn implicit_convert(
    s: &mut Sema<'_, '_>,
    e: ExprId,
    to: QualType,
    ctx: ConvCtx,
) -> Result<ExprId, ConvError> {
    let _ = ctx;
    if s.ast.types.builtin(to) == Some(BuiltinType::Error)
        || s.ast.types.builtin(ty(s, e)) == Some(BuiltinType::Error)
    {
        return Ok(e);
    }
    let e = decay(s, e);
    let from = ty(s, e);
    if s.ast.types.same_unqualified(from, to) {
        return Ok(e);
    }
    let to_u = unqualified_sugar(s, to);
    match scalar_kind(s, e, from, to_u) {
        Some(CastKind::NoOp) => Ok(cast(s, CastKind::NoOp, e, to_u)),
        Some(ck) => Ok(cast(s, ck, e, to_u)),
        None => Err(incompatible(s, from, to)),
    }
}

/// Floating rank: `float` < `double` < `long double`.
fn float_rank(b: BuiltinType) -> u8 {
    match b {
        BuiltinType::Half | BuiltinType::Float16 | BuiltinType::BFloat16 => 0,
        BuiltinType::Float => 1,
        BuiltinType::Double => 2,
        BuiltinType::LongDouble => 3,
        _ => 4,
    }
}

fn to_type(s: &mut Sema<'_, '_>, e: ExprId, to: QualType) -> ExprId {
    let from = ty(s, e);
    if s.ast.types.same_unqualified(from, to) {
        return e;
    }
    let t = &s.ast.types;
    let ck = match (t.is_floating(from), t.is_floating(to)) {
        (true, true) => CastKind::FloatingCast,
        (false, true) => CastKind::IntegralToFloating,
        (true, false) => CastKind::FloatingToIntegral,
        (false, false) => CastKind::IntegralCast,
    };
    cast(s, ck, e, to)
}

/// The unsigned type of the same rank as `b`.
fn unsigned_of(b: BuiltinType) -> QualType {
    match b {
        BuiltinType::Long => QualType::ULONG,
        BuiltinType::LongLong => QualType::ULONGLONG,
        BuiltinType::Int128 => QualType::UINT128,
        _ => QualType::UINT,
    }
}

/// The usual arithmetic conversions [expr.arith.conv].
///
/// # Errors
/// If an operand is not of arithmetic or unscoped enumeration type, or
/// the operands are of different enumeration types (scoped).
pub fn usual_arithmetic_conversions(
    s: &mut Sema<'_, '_>,
    l: ExprId,
    r: ExprId,
) -> Result<(ExprId, ExprId, QualType), ConvError> {
    let (l, r) = (decay(s, l), decay(s, r));
    let (lt, rt) = (ty(s, l), ty(s, r));
    let arith = |s: &Sema<'_, '_>, q: QualType| s.ast.types.is_arithmetic(q) || unscoped_enum(s, q);
    if !arith(s, lt) || !arith(s, rt) {
        if s.ast.types.as_enum(lt).is_some() && s.ast.types.same_unqualified(lt, rt) {
            let q = lt.without_quals();
            return Ok((l, r, q));
        }
        return Err(ConvError {
            id: DiagId::sema_invalid_operands,
            args: vec![s.ty_arg(lt), s.ty_arg(rt)],
        });
    }
    let (lb, rb) = (s.ast.types.builtin(lt), s.ast.types.builtin(rt));
    let lf = lb.is_some_and(BuiltinType::is_floating);
    let rf = rb.is_some_and(BuiltinType::is_floating);
    if lf || rf {
        let target = match (lf, rf) {
            (true, true) => {
                let (lb, rb) = (lb.unwrap_or(BuiltinType::Double), rb.unwrap_or(BuiltinType::Double));
                if float_rank(lb) >= float_rank(rb) { lt } else { rt }
            }
            (true, false) => lt,
            _ => rt,
        };
        let target = s.ast.types.canonical(target).without_quals();
        let l2 = to_type(s, l, target);
        let r2 = to_type(s, r, target);
        return Ok((l2, r2, target));
    }
    let l = integral_promotion(s, l);
    let r = integral_promotion(s, r);
    let (lt, rt) = (s.ast.types.canonical(ty(s, l)).without_quals(), s.ast.types.canonical(ty(s, r)).without_quals());
    if s.ast.types.same_type(lt, rt) {
        return Ok((l, r, lt));
    }
    let lb = s.ast.types.builtin(lt).unwrap_or(BuiltinType::Int);
    let rb = s.ast.types.builtin(rt).unwrap_or(BuiltinType::Int);
    let (ls, rs) = (signed(s, lb), signed(s, rb));
    let (lr, rr) = (rank(lb).unwrap_or(3), rank(rb).unwrap_or(3));
    let target = if ls == rs {
        if lr >= rr { lt } else { rt }
    } else {
        let (sb, sq, ub, uq) = if ls { (lb, lt, rb, rt) } else { (rb, rt, lb, lt) };
        let (sr, ur) = if ls { (lr, rr) } else { (rr, lr) };
        if ur >= sr {
            uq
        } else if width(s, sb) > width(s, ub) {
            sq
        } else {
            unsigned_of(sb)
        }
    };
    let l2 = to_type(s, l, target);
    let r2 = to_type(s, r, target);
    Ok((l2, r2, target))
}

/// Contextual conversion to `bool` [conv.general]/4.
///
/// # Errors
/// If `e` is not contextually convertible to `bool`.
pub fn contextually_convert_to_bool(
    s: &mut Sema<'_, '_>,
    e: ExprId,
) -> Result<ExprId, ConvError> {
    let e = decay(s, e);
    let from = ty(s, e);
    let t = &s.ast.types;
    if t.builtin(from) == Some(BuiltinType::Bool) || t.builtin(from) == Some(BuiltinType::Error) {
        return Ok(e);
    }
    let ck = if t.is_floating(from) {
        CastKind::FloatingToBoolean
    } else if t.is_integral(from) || t.as_enum(from).is_some() {
        CastKind::IntegralToBoolean
    } else if t.is_pointer(from) {
        CastKind::PointerToBoolean
    } else {
        return Err(ConvError {
            id: DiagId::sema_stub_not_contextually_bool,
            args: vec![s.ty_arg(from)],
        });
    };
    Ok(cast(s, ck, e, QualType::BOOL))
}
