//! C assignment constraints (C17 6.5.16.1, the semantics of simple
//! assignment, initialization, argument passing and `return`): clang's
//! `CheckAssignmentConstraints` / `checkPointerTypesForAssignment` and the
//! wording of `DiagnoseAssignmentResult` (spec/landed/sema-conv/WP-1276.md).
//!
//! Only reached in C mode ([`Sema::is_c`]) and only for the contexts that
//! are an assignment in C (not casts, conditions or comparisons). The
//! constraint violations clang accepts with a diagnostic (an incompatible
//! pointer, an integer to pointer conversion under `-Wno-int-conversion`)
//! come back as a [`ConvError`] carrying the converted expression in
//! `recovered`: the caller emits the diagnostic and keeps the value.

use kiln_ast::decl::DeclKind;
use kiln_ast::expr::{CastKind, ExprKind, UnaryOp, ValueCategory};
use kiln_ast::types::{BuiltinType, TypeView};
use kiln_ast::{ExprId, QualType};
use kiln_diag::DiagId;

use super::{ConvCtx, ConvError, array_decayed, cast_to, decay, ty};
use crate::Sema;

/// clang's `AssignmentAction` select of the C assignment diagnostics:
/// assigning, passing, returning, converting, initializing.
fn action(ctx: ConvCtx) -> Option<&'static str> {
    match ctx {
        ConvCtx::Assignment => Some("0"),
        ConvCtx::Argument => Some("1"),
        ConvCtx::Return => Some("2"),
        ConvCtx::Initialization | ConvCtx::DirectInitialization => Some("4"),
        ConvCtx::Comparison | ConvCtx::Condition | ConvCtx::Cast => None,
    }
}

/// Whether `id` is one of the C assignment diagnostics (located as clang
/// `DiagnoseAssignmentResult` locates them: at the operator of an
/// assignment, at the start of an argument or `return` operand).
pub fn is_c_assign_diag(id: DiagId) -> bool {
    matches!(
        id,
        DiagId::sema_c_assign_discards_nested_qualifiers
            | DiagId::sema_c_assign_discards_qualifiers
            | DiagId::sema_c_assign_incompatible
            | DiagId::sema_c_assign_incompatible_function_pointer
            | DiagId::sema_c_assign_incompatible_pointer
            | DiagId::sema_c_assign_incompatible_pointer_default_error
            | DiagId::sema_c_assign_int_to_pointer
            | DiagId::sema_c_assign_pointer_sign
            | DiagId::sema_c_assign_pointer_to_int
    )
}

/// The outcome of `checkPointerTypesForAssignment`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum PtrCheck {
    Compatible,
    DiscardsQualifiers,
    PointerSign,
    NestedQualifiers,
    FunctionPointer,
    Incompatible,
}

const DEPTH: u32 = 64;

/// C17 6.2.7 type compatibility of `a` and `b`, qualifiers included.
pub(crate) fn compatible(s: &Sema<'_, '_>, a: QualType, b: QualType) -> bool {
    compatible_at(s, a, b, DEPTH)
}

fn compatible_at(s: &Sema<'_, '_>, a: QualType, b: QualType, depth: u32) -> bool {
    let types = &s.ast.types;
    let (a, b) = (types.canonical(a), types.canonical(b));
    if a == b {
        return true;
    }
    if depth == 0 || a.quals() != b.quals() {
        return false;
    }
    compatible_unqual(s, a.without_quals(), b.without_quals(), depth - 1)
}

/// The integer type an enumeration is compatible with (C17 6.7.2.2p4).
fn enum_int(s: &Sema<'_, '_>, q: QualType) -> Option<QualType> {
    let t = super::rank::enum_data(s, q)?.int_type?;
    Some(s.ast.types.canonical(t).without_quals())
}

fn compatible_unqual(s: &Sema<'_, '_>, a: QualType, b: QualType, depth: u32) -> bool {
    let types = &s.ast.types;
    match (types.canonical_view(a), types.canonical_view(b)) {
        (TypeView::Pointer(x), TypeView::Pointer(y))
        | (TypeView::Complex(x), TypeView::Complex(y))
        | (TypeView::Atomic(x), TypeView::Atomic(y)) => compatible_at(s, x, y, depth),
        (TypeView::Enum(_), TypeView::Builtin(_) | TypeView::BitInt { .. }) => {
            enum_int(s, a) == Some(b)
        }
        (TypeView::Builtin(_) | TypeView::BitInt { .. }, TypeView::Enum(_)) => {
            enum_int(s, b) == Some(a)
        }
        (
            TypeView::ConstantArray(_)
            | TypeView::IncompleteArray(_)
            | TypeView::VariableArray { .. },
            TypeView::ConstantArray(_)
            | TypeView::IncompleteArray(_)
            | TypeView::VariableArray { .. },
        ) => {
            let size = |v: TypeView| match v {
                TypeView::ConstantArray(c) => Some(c.size),
                _ => None,
            };
            let (sa, sb) = (size(types.canonical_view(a)), size(types.canonical_view(b)));
            if let (Some(x), Some(y)) = (sa, sb)
                && x != y
            {
                return false;
            }
            match (types.element(a), types.element(b)) {
                (Some(x), Some(y)) => compatible_at(s, x, y, depth),
                _ => false,
            }
        }
        (TypeView::Function(f), TypeView::Function(g))
            if f.info().no_proto() || g.info().no_proto() =>
        {
            // C17 6.7.6.3p15: against a type without a prototype. A K&R
            // definition's parameters only check redeclarations (clang
            // refers to the function as `ret ()`, DR 316).
            if !compatible_at(s, f.ret(), g.ret(), depth) {
                return false;
            }
            let p = if f.info().no_proto() { g } else { f };
            p.info().no_proto()
                || !p.variadic()
                    && types.params(&p).iter().all(|&t| s.va_arg_promotion(t).is_none())
        }
        (TypeView::Function(f), TypeView::Function(g)) => {
            if !compatible_at(s, f.ret(), g.ret(), depth)
                || f.variadic() != g.variadic()
                || f.num_params() != g.num_params()
            {
                return false;
            }
            let (pf, pg) = (types.params(&f), types.params(&g));
            pf.iter().zip(pg).all(|(&x, &y)| {
                let (x, y) =
                    (types.canonical(x).without_quals(), types.canonical(y).without_quals());
                compatible_at(s, x, y, depth)
            })
        }
        _ => false,
    }
}

/// clang's sign-insensitive pointee: plain `char` and the signed integer
/// types map to their unsigned counterpart. An enumeration whose underlying type is signed maps to that type's
/// counterpart (clang `hasSignedIntegerRepresentation`); another stays.
fn sign_insensitive(s: &mut Sema<'_, '_>, q: QualType) -> QualType {
    if s.ast.types.builtin(q) == Some(BuiltinType::Char) {
        return QualType::UCHAR;
    }
    let base = enum_int(s, q).unwrap_or(q);
    if super::rank::int_signed(s, base) { super::rank::unsigned_counterpart(s, base) } else { q }
}

/// clang's `checkPointerTypesForAssignment` of `rhs` to `lhs`, both
/// (canonical) pointer types.
fn check_pointers(s: &mut Sema<'_, '_>, lhs: QualType, rhs: QualType) -> PtrCheck {
    let types = &s.ast.types;
    let (Some(lp), Some(rp)) = (types.pointee(lhs), types.pointee(rhs)) else {
        return PtrCheck::Incompatible;
    };
    let (lp, rp) = (types.canonical(lp), types.canonical(rp));
    let mut res = PtrCheck::Compatible;
    if !lp.quals().contains(rp.quals()) {
        res = PtrCheck::DiscardsQualifiers;
    }
    // 6.5.16.1p1: a pointer to void and a pointer to an object type (a
    // function pointer as clang's extension, silently).
    if types.is_void(lp) || types.is_void(rp) {
        return res;
    }
    let (lu, ru) = (lp.without_quals(), rp.without_quals());
    if compatible(s, lu, ru) {
        return res;
    }
    let (ls, rs) = (sign_insensitive(s, lu), sign_insensitive(s, ru));
    let types = &s.ast.types;
    if types.canonical(ls) == types.canonical(rs) && types.is_integral(ls) {
        return if res == PtrCheck::Compatible { PtrCheck::PointerSign } else { res };
    }
    if types.is_pointer(lu) && types.is_pointer(ru) {
        let (mut l, mut r) = (lu, ru);
        for _ in 0..DEPTH {
            match (types.pointee(l), types.pointee(r)) {
                (Some(x), Some(y)) => {
                    l = types.canonical(x).without_quals();
                    r = types.canonical(y).without_quals();
                }
                _ => break,
            }
        }
        if l == r {
            return PtrCheck::NestedQualifiers;
        }
    }
    if types.is_function(lu) && types.is_function(ru) {
        return PtrCheck::FunctionPointer;
    }
    PtrCheck::Incompatible
}

/// Whether `e` is an integer constant expression in C's sense (C17 6.6p6):
/// no object is read (an enumeration constant is not an object).
pub(crate) fn is_c_ice(s: &Sema<'_, '_>, e: ExprId) -> bool {
    let mut stack = vec![e];
    let mut seen = 0u32;
    while let Some(x) = stack.pop() {
        seen += 1;
        if seen > 4096 {
            return false;
        }
        let n = s.ast.exprs.get(x);
        match n.kind() {
            ExprKind::UnaryExprOrTypeTraitExpr | ExprKind::OffsetOfExpr => continue,
            ExprKind::DeclRefExpr => {
                if s.kind_of(s.ast.exprs.decl(x)) != DeclKind::EnumConstant {
                    return false;
                }
            }
            ExprKind::UnaryOperator => {
                if matches!(s.ast.exprs.unary_op(x), UnaryOp::Deref | UnaryOp::AddrOf) {
                    return false;
                }
            }
            ExprKind::IntegerLiteral
            | ExprKind::CharacterLiteral
            | ExprKind::FloatingLiteral
            | ExprKind::CXXBoolLiteralExpr
            | ExprKind::ParenExpr
            | ExprKind::ImplicitCastExpr
            | ExprKind::CStyleCastExpr
            | ExprKind::BinaryOperator
            | ExprKind::ConditionalOperator
            | ExprKind::ConstantExpr
            | ExprKind::GenericSelectionExpr
            | ExprKind::ChooseExpr => {}
            _ => return false,
        }
        s.ast.exprs.children_into(x, &s.ast.lists, &mut stack);
    }
    true
}

/// C17 6.3.2.3p3: whether `e` is a null pointer constant, an integer
/// constant expression of value 0 (or one cast to `void *`), or C23
/// `nullptr`. `Some(true)` for a literal `0` (clang's `NPCK_ZeroLiteral`),
/// `Some(false)` for another zero expression, which warns
/// `-Wnon-literal-null-conversion`.
pub(crate) fn null_pointer_constant(s: &mut Sema<'_, '_>, e: ExprId) -> Option<bool> {
    if s.ast.types.builtin(ty(s, e)) == Some(BuiltinType::NullPtr) {
        return Some(true);
    }
    let mut cur = e;
    for _ in 0..256 {
        let n = *s.ast.exprs.get(cur);
        match n.kind() {
            ExprKind::ParenExpr => cur = s.ast.exprs.operand(cur),
            ExprKind::CStyleCastExpr | ExprKind::ImplicitCastExpr
                if s.ast.types.pointee(n.ty()).is_some_and(|p| {
                    s.ast.types.is_void(p) && s.ast.types.canonical(p).quals().is_empty()
                }) =>
            {
                cur = s.ast.exprs.operand(cur);
            }
            _ => break,
        }
    }
    let t = ty(s, cur);
    if !(s.ast.types.is_integral(t) || s.ast.types.as_enum(t).is_some()) {
        return None;
    }
    let n = *s.ast.exprs.get(cur);
    if n.kind() == ExprKind::IntegerLiteral {
        return (s.ast.exprs.int_value(cur) == Some(0)).then_some(true);
    }
    if !is_c_ice(s, cur) {
        return None;
    }
    (s.fold_int(cur) == Some(0)).then_some(false)
}

/// clang's `ConversionFixItGenerator::compareTypesSimple`: `from` and
/// `to` (their pointees, when both are pointers) are the same type, `to`
/// at least as qualified.
fn fix_compare(s: &Sema<'_, '_>, from: QualType, to: QualType) -> bool {
    let types = &s.ast.types;
    let (mut f, mut t) = (types.canonical(from), types.canonical(to));
    if !t.quals().contains(f.quals()) {
        return false;
    }
    if let (Some(x), Some(y)) = (types.pointee(f), types.pointee(t)) {
        f = types.canonical(x);
        t = types.canonical(y);
    }
    f.without_quals() == t.without_quals() && t.quals().contains(f.quals())
}

/// clang's `ConversionFixItGenerator::tryToFixConversion` select for `e`
/// of type `from` (as written) converted to `to`: none, "dereference with
/// *", "take the address with &", "remove *", "remove &".
fn fix_hint(s: &mut Sema<'_, '_>, e: ExprId, from: QualType, to: QualType) -> &'static str {
    let mut inner = e;
    for _ in 0..256 {
        if s.ast.exprs.get(inner).kind() != ExprKind::ImplicitCastExpr {
            break;
        }
        inner = s.ast.exprs.operand(inner);
    }
    let op = (s.ast.exprs.get(inner).kind() == ExprKind::UnaryOperator)
        .then(|| s.ast.exprs.unary_op(inner));
    if let Some(p) = s.ast.types.pointee(from)
        && s.ast.types.is_pointer(from)
        && fix_compare(s, p, to)
    {
        if null_pointer_constant(s, e).is_some() {
            return "0";
        }
        return if op == Some(UnaryOp::AddrOf) { "4" } else { "1" };
    }
    if s.ast.types.is_pointer(to) {
        if s.ast.exprs.get(inner).value_category() != ValueCategory::LValue {
            return "0";
        }
        let p = s.ast.types.pointer_to(from);
        if fix_compare(s, p, to) {
            return if op == Some(UnaryOp::Deref) { "3" } else { "2" };
        }
    }
    "0"
}

/// The type of `e` after array-to-pointer and function-to-pointer decay.
fn decayed_type(s: &mut Sema<'_, '_>, e: ExprId) -> QualType {
    let n = *s.ast.exprs.get(e);
    let t = n.ty();
    if s.ast.types.is_array(t) {
        // A prvalue array (a compound literal) decays too: [`decay`]
        // materializes it (C17 6.3.2.1p3).
        array_decayed(s, t)
    } else if s.ast.types.is_function(t) {
        s.ast.types.pointer_to(t)
    } else {
        t
    }
}

/// The C assignment diagnostic `id` for `e` (of type `from` as written,
/// before decay) converted to `to` in action `act`.
fn error(
    s: &mut Sema<'_, '_>,
    id: DiagId,
    e: ExprId,
    from: QualType,
    to: QualType,
    act: &'static str,
    hint: Option<&'static str>,
) -> ConvError {
    let (fa, ta) = (s.ty_arg(from), s.ty_arg(to));
    // Assigning and initializing name the destination first.
    let (a0, a1) = if matches!(act, "0" | "4") { (ta, fa) } else { (fa, ta) };
    let mut args = vec![a0, a1, act.to_string()];
    if let Some(h) = hint {
        args.push(h.into());
    }
    let mut err = ConvError::new(id, args);
    err.range = Some(s.expr_range(e));
    if let Some(op @ ("1" | "2")) = hint {
        err.fixits = insertion_fixits(s, e, if op == "1" { "*" } else { "&" });
    }
    err
}

/// clang's `tryToFixConversion` insertions: `op` before `e`, wrapped in
/// parentheses unless `e` (implicit casts stripped) is a postfix, primary,
/// cast or unary expression.
fn insertion_fixits(s: &Sema<'_, '_>, e: ExprId, op: &str) -> Vec<kiln_diag::FixIt> {
    let mut inner = e;
    for _ in 0..256 {
        if s.ast.exprs.get(inner).kind() != ExprKind::ImplicitCastExpr {
            break;
        }
        inner = s.ast.exprs.operand(inner);
    }
    let begin = s.begin_loc(e);
    let bare = matches!(
        s.ast.exprs.get(inner).kind(),
        ExprKind::ArraySubscriptExpr
            | ExprKind::CallExpr
            | ExprKind::DeclRefExpr
            | ExprKind::MemberExpr
            | ExprKind::ParenExpr
            | ExprKind::UnaryOperator
            | ExprKind::CStyleCastExpr
    );
    if bare {
        return vec![kiln_diag::FixIt::insert(begin, op)];
    }
    let last = s.expr_end_loc(e);
    let end = s.token_end(last).unwrap_or(last);
    vec![kiln_diag::FixIt::insert(begin, format!("{op}(")), kiln_diag::FixIt::insert(end, ")")]
}

/// [`super::implicit_convert`] in C for the cases 6.5.16.1 constrains:
/// a pointer target, a pointer source of integer target, and a structure
/// or union on either side. `None` leaves the conversion to the standard
/// conversion path (arithmetic conversions).
#[allow(clippy::too_many_lines, reason = "clang's CheckAssignmentConstraints cases in its order")]
pub(crate) fn convert(
    s: &mut Sema<'_, '_>,
    e: ExprId,
    to: QualType,
    ctx: ConvCtx,
) -> Option<Result<ExprId, ConvError>> {
    let act = action(ctx)?;
    let from = ty(s, e);
    let types = &s.ast.types;
    // A reference parameter is a builtin's (`__builtin_va_start`'s
    // `va_list &`): reference binding, not assignment.
    if types.is_reference(to) {
        return None;
    }
    if types.as_atomic(to).is_some() || types.as_atomic(from).is_some() {
        return None;
    }
    if s.vector_parts(from).is_some() || s.vector_parts(to).is_some() {
        return None;
    }
    let to_rec = types.as_record(to).is_some();
    let from_rec = types.as_record(from).is_some();
    if to_rec || from_rec {
        if to_rec && from_rec && compatible(s, from.without_quals(), to.without_quals()) {
            // C17 6.3.2.1p2: the structure's value (clang's
            // `DefaultLvalueConversion` of a record in C). An assigned
            // prvalue is its own object first (a compound literal may read
            // the target, C17 6.5.2.5p4), never built in place over it.
            let mut e = e;
            if !s.ast.exprs.get(e).value_category().is_glvalue() {
                if !matches!(ctx, ConvCtx::Assignment) {
                    return Some(Ok(e));
                }
                e = s.materialize_temporary(e, from, false);
            }
            let u = super::strip_cv(s, from);
            let x =
                super::raw_cast(s, CastKind::LValueToRValue, e, u, ValueCategory::PRValue, None);
            return Some(Ok(x));
        }
        if to_rec
            && ctx == ConvCtx::Argument
            && let Some(x) = s.transparent_union_argument(e, to)
        {
            return Some(Ok(x));
        }
        let err = error(s, DiagId::sema_c_assign_incompatible, e, from, to, act, Some("0"));
        return Some(Err(err));
    }
    let from_d = decayed_type(s, e);
    let types = &s.ast.types;
    let to_ptr = types.is_pointer(to);
    let from_ptr = types.is_pointer(from_d);
    let int_like = |q: QualType| types.is_integral(q) || types.as_enum(q).is_some();
    let from_int = int_like(from_d);
    let to_int = int_like(to) && types.builtin(to) != Some(BuiltinType::Bool);
    if to_ptr {
        let to_c = s.ast.types.canonical(to);
        if !from_ptr && let Some(literal) = null_pointer_constant(s, e) {
            let x = decay(s, e);
            let x = cast_to(s, CastKind::NullToPointer, x, to, None);
            if !literal {
                let at = s.ast.exprs.get(e).loc();
                let d = s.diag(DiagId::sema_c_non_literal_null, at).arg(s.ty_arg(to));
                let d = d.range(s.expr_range(e));
                s.emit(d);
            }
            return Some(Ok(x));
        }
        if from_ptr {
            let from_c = s.ast.types.canonical(from_d);
            let chk = check_pointers(s, to_c, from_c);
            let x = decay(s, e);
            let same = {
                let types = &s.ast.types;
                match (types.pointee(to_c), types.pointee(from_c)) {
                    (Some(a), Some(b)) => types.same_unqualified(a, b),
                    _ => false,
                }
            };
            let ck = if same { CastKind::NoOp } else { CastKind::BitCast };
            let x = cast_to(s, ck, x, to, None);
            let (id, hint) = match chk {
                PtrCheck::Compatible => return Some(Ok(x)),
                PtrCheck::DiscardsQualifiers => (DiagId::sema_c_assign_discards_qualifiers, None),
                PtrCheck::NestedQualifiers => {
                    (DiagId::sema_c_assign_discards_nested_qualifiers, None)
                }
                PtrCheck::PointerSign => {
                    let types = &s.ast.types;
                    let plain = |q: QualType| {
                        types.pointee(q).and_then(|p| types.builtin(p)) == Some(BuiltinType::Char)
                    };
                    let sel = if plain(to_c) == plain(from_c) { "0" } else { "1" };
                    (DiagId::sema_c_assign_pointer_sign, Some(sel))
                }
                PtrCheck::FunctionPointer => {
                    let h = fix_hint(s, e, from, to);
                    (DiagId::sema_c_assign_incompatible_function_pointer, Some(h))
                }
                PtrCheck::Incompatible => {
                    let h = fix_hint(s, e, from, to);
                    // clang 22 makes `-Wincompatible-pointer-types` an
                    // error by default; clang 19 warns (WP-1285).
                    let id = if s.clang_compat >= 22 {
                        DiagId::sema_c_assign_incompatible_pointer_default_error
                    } else {
                        DiagId::sema_c_assign_incompatible_pointer
                    };
                    (id, Some(h))
                }
            };
            let mut err = error(s, id, e, from, to, act, hint);
            err.recovered = Some(x);
            return Some(Err(err));
        }
        if from_int {
            let h = fix_hint(s, e, from, to);
            let x = decay(s, e);
            let x = cast_to(s, CastKind::IntegralToPointer, x, to, None);
            let mut err = error(s, DiagId::sema_c_assign_int_to_pointer, e, from, to, act, Some(h));
            err.recovered = Some(x);
            return Some(Err(err));
        }
        let h = fix_hint(s, e, from, to);
        return Some(Err(error(s, DiagId::sema_c_assign_incompatible, e, from, to, act, Some(h))));
    }
    if from_ptr {
        if to_int {
            let h = fix_hint(s, e, from, to);
            let x = decay(s, e);
            let x = cast_to(s, CastKind::PointerToIntegral, x, to, None);
            let mut err = error(s, DiagId::sema_c_assign_pointer_to_int, e, from, to, act, Some(h));
            err.recovered = Some(x);
            return Some(Err(err));
        }
        if s.ast.types.builtin(to) == Some(BuiltinType::Bool) {
            return None;
        }
        let h = fix_hint(s, e, from, to);
        return Some(Err(error(s, DiagId::sema_c_assign_incompatible, e, from, to, act, Some(h))));
    }
    None
}

/// One member of a transparent union parameter
/// ([`Sema::transparent_union_argument`], clang
/// `CheckTransparentUnionArgumentConstraints`): `e` converted to member
/// type `fty` when that is a compatible assignment, else `None` (with no
/// diagnostic). A pointer member also takes any `void *` and a null
/// pointer constant.
pub(crate) fn argument_member(s: &mut Sema<'_, '_>, e: ExprId, fty: QualType) -> Option<ExprId> {
    let from = ty(s, e);
    let from_d = decayed_type(s, e);
    let types = &s.ast.types;
    let arith = |q: QualType| types.is_arithmetic(q) || types.as_enum(q).is_some();
    if types.is_pointer(fty) {
        if types.is_pointer(from_d) {
            let to_c = types.canonical(fty);
            let from_c = types.canonical(from_d);
            let void = types.pointee(from_c).is_some_and(|p| types.is_void(p));
            if !void && check_pointers(s, to_c, from_c) != PtrCheck::Compatible {
                return None;
            }
            let x = decay(s, e);
            return Some(cast_to(s, CastKind::BitCast, x, fty, None));
        }
        null_pointer_constant(s, e)?;
        let x = decay(s, e);
        return Some(cast_to(s, CastKind::NullToPointer, x, fty, None));
    }
    if types.as_record(fty).is_some() {
        return match convert(s, e, fty, ConvCtx::Argument) {
            Some(Ok(x)) => Some(x),
            _ => None,
        };
    }
    if arith(fty) && arith(from_d) && s.vector_parts(from).is_none() {
        return super::implicit_convert(s, e, fty, ConvCtx::Argument).ok();
    }
    None
}
