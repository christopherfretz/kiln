//! Implicit conversions (\[conv\], [over.best.ics], [expr.arith.conv]).
//!
//! The entry points take an expression already in the AST and return the
//! expression with the conversion's `ImplicitCastExpr`s wrapped around it,
//! shaped exactly as clang shapes them (see `spec/sema-conv.md` for the
//! cast-target rules). Failures come back as a [`ConvError`] that the
//! caller emits at the expression; nothing here panics on user input.
//!
//! - [`lvalue_to_rvalue`], [`decay`], [`integral_promotion`] and
//!   [`usual_unary_conversions`]: the operand conversions;
//! - [`standard_conversion`] computes a standard conversion sequence
//!   ([over.ics.scs]), [`apply_standard_conversion`] performs it and
//!   [`compare_standard_conversions`] ranks two ([over.ics.rank]);
//! - [`implicit_convert`]: copy-initialization of a non-class object from
//!   an expression, with clang's diagnostics per [`ConvCtx`];
//! - [`usual_arithmetic_conversions`] ([expr.arith.conv]);
//! - [`contextually_convert_to_bool`] ([conv.general]/4);
//! - [`narrowing_kind`] ([dcl.init.list]/7).
#![allow(clippy::many_single_char_names, reason = "s/e/t/q/n: sema, expr, type, qualtype, node")]

mod apply;
mod arith;
pub(crate) mod bases;
pub(crate) mod narrow;
mod qual;
pub(crate) mod rank;
pub(crate) mod scs;
pub(crate) mod soft;

use kiln_ast::expr::{CastKind, ExprKind, ObjectKind, ValueCategory};
use kiln_ast::types::{Quals, TypeView};
use kiln_ast::{ExprId, QualType};
use kiln_diag::DiagId;

use crate::Sema;

pub use apply::apply_standard_conversion;
pub use arith::{ArithConvKind, usual_arithmetic_conversions_kind};
pub use bases::{BasePath, BaseSearch, find_base, is_derived_from, path_list};
pub use narrow::{Narrowing, narrowing_diag, narrowing_kind};
pub use qual::{
    QualRelation, is_function_pointer_conversion, is_qualification_convertible,
    qualification_relation, similar,
};
pub use rank::{
    FloatOrder, float_order, int_signed, int_width, integer_rank, is_arithmetic_or_unscoped_enum,
    is_integral_or_unscoped_enum, is_scoped_enum, is_unscoped_enum, promoted_type,
};
pub use scs::{
    ConvRank, FirstStep, FixedEnumPromotion, IcsCompare, SecondStep, StandardConversion, ThirdStep,
    compare_standard_conversions, standard_conversion, standard_conversion_from,
};

/// Where a conversion happens: selects the conversions allowed and
/// clang's wording when there is none.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConvCtx {
    /// Copy-initialization of a variable or member ([dcl.init.general]/15).
    Initialization,
    /// Direct-initialization ([dcl.init.general]/16): additionally allows
    /// `std::nullptr_t` to `bool` ([conv.bool]).
    DirectInitialization,
    /// The right operand of a simple assignment ([expr.assign]/3).
    Assignment,
    /// A function argument ([expr.call]/7).
    Argument,
    /// A `return` operand ([stmt.return]/2).
    Return,
    /// An operand of a comparison.
    Comparison,
    /// A condition: contextual conversion to `bool`, a direct-
    /// initialization ([conv.general]/4).
    Condition,
    /// The implicit part of an explicit cast.
    Cast,
}

impl ConvCtx {
    fn is_direct(self) -> bool {
        matches!(self, ConvCtx::DirectInitialization | ConvCtx::Condition | ConvCtx::Cast)
    }
}

/// A conversion that cannot be made: `id` with `args`, to be emitted at
/// the expression.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ConvError {
    /// The diagnostic.
    pub id: DiagId,
    /// Its arguments, in order (type arguments pre-quoted, as
    /// [`Sema::ty_arg`] prints them).
    pub args: Vec<String>,
}

impl ConvError {
    pub(crate) fn new(id: DiagId, args: Vec<String>) -> ConvError {
        ConvError { id, args }
    }

    /// The diagnostic, with its caret at `e`'s location.
    pub fn diag(&self, s: &Sema<'_, '_>, e: ExprId) -> kiln_diag::Diag {
        let loc = s.ast.exprs.get(e).loc();
        self.args.iter().fold(s.diag(self.id, loc), kiln_diag::Diag::arg)
    }
}

pub(crate) fn ty(s: &Sema<'_, '_>, e: ExprId) -> QualType {
    s.ast.exprs.get(e).ty()
}

fn is_error(s: &Sema<'_, '_>, q: QualType) -> bool {
    s.ast.types.canonical(q).without_quals() == QualType::ERROR
}

/// `q` without cv-qualifiers, keeping as much sugar as possible: clang's
/// `getUnqualifiedType` (sugar carrying qualifiers is looked through).
pub fn strip_cv(s: &Sema<'_, '_>, q: QualType) -> QualType {
    let mut cur = q;
    for _ in 0..256 {
        let u = cur.without_quals();
        if s.ast.types.canonical(u).quals().is_empty() {
            return u;
        }
        let next = s.ast.types.desugar_once(u);
        if next == u {
            break;
        }
        cur = next;
    }
    s.ast.types.canonical(q).without_quals()
}

/// An implicit prvalue cast of `e` to `to`, skipped when `e`'s type is
/// already canonically `to` (clang's `ImpCastExprToType`).
pub(crate) fn cast_to(
    s: &mut Sema<'_, '_>,
    ck: CastKind,
    e: ExprId,
    to: QualType,
    path: Option<kiln_ast::ListId>,
) -> ExprId {
    let from = ty(s, e);
    if s.ast.types.canonical(from) == s.ast.types.canonical(to) {
        return e;
    }
    // clang retypes an implicit cast of the same kind in place instead of
    // stacking a second one (`(unsigned char)i + 1L` converts the
    // `unsigned char` straight to `long`); the replaced node is dropped.
    let n = s.ast.exprs.get(e);
    if n.kind() == ExprKind::ImplicitCastExpr
        && n.value_category() == ValueCategory::PRValue
        && s.ast.exprs.cast_kind(e) == ck
        && path.is_none()
        && s.ast.exprs.cast_path(e).is_none()
    {
        let inner = s.ast.exprs.operand(e);
        return raw_cast(s, ck, inner, to, ValueCategory::PRValue, None);
    }
    raw_cast(s, ck, e, to, ValueCategory::PRValue, path)
}

/// An implicit cast, unconditionally.
pub(crate) fn raw_cast(
    s: &mut Sema<'_, '_>,
    ck: CastKind,
    e: ExprId,
    to: QualType,
    vk: ValueCategory,
    path: Option<kiln_ast::ListId>,
) -> ExprId {
    let loc = s.ast.exprs.get(e).loc();
    s.ast.builder().cast(
        ExprKind::ImplicitCastExpr,
        ck,
        e,
        to,
        vk,
        path,
        loc,
        kiln_ast::expr::ExprFlags::NONE,
    )
}

/// The pointer type an array of type `q` decays to ([conv.array]): the
/// element type as written, carrying the array's qualifiers.
pub(crate) fn array_decayed(s: &mut Sema<'_, '_>, q: QualType) -> QualType {
    let mut cur = q;
    let mut quals = Quals::NONE;
    for _ in 0..256 {
        quals |= cur.quals();
        match s.ast.types.view(cur.ty()) {
            TypeView::ConstantArray(a) => {
                let elem = s.ast.types.qualify(a.elem, quals);
                return s.ast.types.pointer_to(elem);
            }
            TypeView::IncompleteArray(elem)
            | TypeView::VariableArray { elem, .. }
            | TypeView::DependentSizedArray { elem, .. } => {
                let elem = s.ast.types.qualify(elem, quals);
                return s.ast.types.pointer_to(elem);
            }
            _ => {
                let next = s.ast.types.desugar_once(cur);
                if next == cur {
                    break;
                }
                cur = next;
            }
        }
    }
    let c = s.ast.types.canonical(q);
    let elem = s.ast.types.element(c).unwrap_or(QualType::ERROR);
    let elem = s.ast.types.qualify(elem, c.quals());
    s.ast.types.pointer_to(elem)
}

/// The lvalue-to-rvalue conversion ([conv.lval]): a glvalue of non-class,
/// non-array, non-function type `cv T` becomes a prvalue of `T`
/// (`LValueToRValue`; `NullToPointer` for `std::nullptr_t`, as clang
/// does). Identity on prvalues, class types, arrays and functions.
pub fn lvalue_to_rvalue(s: &mut Sema<'_, '_>, e: ExprId) -> ExprId {
    let n = *s.ast.exprs.get(e);
    if !n.value_category().is_glvalue() {
        return e;
    }
    let t = n.ty();
    let types = &s.ast.types;
    if is_error(s, t)
        || types.is_dependent(t)
        || types.as_record(t).is_some()
        || types.is_void(t)
        || types.is_array(t)
        || types.is_function(t)
    {
        return e;
    }
    let to = strip_cv(s, t);
    let ck = if types.builtin(t) == Some(kiln_ast::types::BuiltinType::NullPtr) {
        CastKind::NullToPointer
    } else {
        CastKind::LValueToRValue
    };
    raw_cast(s, ck, e, to, ValueCategory::PRValue, None)
}

/// The array-to-pointer ([conv.array]) and function-to-pointer
/// ([conv.func]) conversions, else the lvalue-to-rvalue conversion
/// (clang's `DefaultFunctionArrayLvalueConversion`).
pub fn decay(s: &mut Sema<'_, '_>, e: ExprId) -> ExprId {
    let n = *s.ast.exprs.get(e);
    let t = n.ty();
    if s.ast.types.is_dependent(t) {
        return e;
    }
    if s.check_builtin_fn_use(e) {
        let loc = s.eloc(e);
        return s.recover_err(&[e], loc);
    }
    if s.ast.types.is_function(t) {
        if n.value_category() == ValueCategory::LValue {
            let p = s.ast.types.pointer_to(t);
            return raw_cast(
                s,
                CastKind::FunctionToPointerDecay,
                e,
                p,
                ValueCategory::PRValue,
                None,
            );
        }
        return e;
    }
    if s.ast.types.is_array(t) {
        if n.value_category().is_glvalue() {
            let p = array_decayed(s, t);
            return raw_cast(s, CastKind::ArrayToPointerDecay, e, p, ValueCategory::PRValue, None);
        }
        return e;
    }
    lvalue_to_rvalue(s, e)
}

/// [`decay`] of a type-dependent operand, as clang's
/// `DefaultFunctionArrayLvalueConversion` does it: arrays and functions
/// decay, and a glvalue of (dependent) pointer or pointer-to-member type is
/// converted to a prvalue; other dependent types are left alone.
pub fn decay_dependent(s: &mut Sema<'_, '_>, e: ExprId) -> ExprId {
    let n = *s.ast.exprs.get(e);
    let t = n.ty();
    if !s.ast.types.is_dependent(t) {
        return decay(s, e);
    }
    if !n.value_category().is_glvalue() {
        return e;
    }
    let types = &s.ast.types;
    if types.is_function(t) {
        if n.value_category() == ValueCategory::LValue {
            let p = s.ast.types.pointer_to(t);
            return raw_cast(
                s,
                CastKind::FunctionToPointerDecay,
                e,
                p,
                ValueCategory::PRValue,
                None,
            );
        }
        return e;
    }
    if types.is_array(t) {
        let p = array_decayed(s, t);
        return raw_cast(s, CastKind::ArrayToPointerDecay, e, p, ValueCategory::PRValue, None);
    }
    if !matches!(
        types.canonical_kind(t),
        kiln_ast::types::TypeKind::Pointer | kiln_ast::types::TypeKind::MemberPointer
    ) {
        return e;
    }
    let to = strip_cv(s, t);
    raw_cast(s, CastKind::LValueToRValue, e, to, ValueCategory::PRValue, None)
}

/// The value of the integral constant expression `w` (a bit-field width:
/// a `ConstantExpr`, a literal, or casts and parentheses around one).
fn eval_width(s: &Sema<'_, '_>, w: ExprId) -> Option<u32> {
    let mut cur = w;
    for _ in 0..64 {
        let n = s.ast.exprs.get(cur);
        match n.kind() {
            ExprKind::IntegerLiteral => {
                return s.ast.exprs.int_value(cur).and_then(|v| u32::try_from(v).ok());
            }
            ExprKind::ConstantExpr => {
                if let Some(v) = s.ast.exprs.constant_value(cur) {
                    return match s.ast.consts.scalar(v)? {
                        kiln_ast::consts::Scalar::Int { bits, signed, .. } => {
                            if signed {
                                bits.to_i128().and_then(|v| u32::try_from(v).ok())
                            } else {
                                bits.to_u128().and_then(|v| u32::try_from(v).ok())
                            }
                        }
                        kiln_ast::consts::Scalar::Bool { value, .. } => Some(u32::from(value)),
                        _ => None,
                    };
                }
                cur = s.ast.exprs.operand(cur);
            }
            ExprKind::ParenExpr | ExprKind::ImplicitCastExpr => cur = s.ast.exprs.operand(cur),
            _ => return None,
        }
    }
    None
}

/// The width of the bit-field `e` designates, if any (through
/// parentheses and no-op/lvalue-to-rvalue casts; clang's
/// `getSourceBitField`).
pub fn bit_field_width(s: &Sema<'_, '_>, e: ExprId) -> Option<u32> {
    let mut cur = e;
    for _ in 0..256 {
        let n = s.ast.exprs.get(cur);
        match n.kind() {
            ExprKind::ParenExpr => cur = s.ast.exprs.operand(cur),
            ExprKind::ImplicitCastExpr
                if matches!(
                    s.ast.exprs.cast_kind(cur),
                    CastKind::NoOp | CastKind::LValueToRValue
                ) =>
            {
                cur = s.ast.exprs.operand(cur);
            }
            ExprKind::MemberExpr if n.object_kind() == ObjectKind::BitField => {
                let f = s.ast.exprs.decl(cur);
                let w = s.ast.decls.field(f)?.bit_width?;
                return eval_width(s, w);
            }
            _ => return None,
        }
    }
    None
}

/// The integral promotions ([conv.prom]) of an operand: decays it, then
/// promotes a promotable integer, unscoped enumeration or bit-field
/// (`IntegralCast`). Identity for other types.
pub fn integral_promotion(s: &mut Sema<'_, '_>, e: ExprId) -> ExprId {
    let bw = bit_field_width(s, e);
    let e = decay(s, e);
    promote_int(s, e, bw)
}

fn promote_int(s: &mut Sema<'_, '_>, e: ExprId, bw: Option<u32>) -> ExprId {
    let t = ty(s, e);
    if !is_integral_or_unscoped_enum(s, t) {
        return e;
    }
    match promoted_type(s, t, bw) {
        Some(p) => cast_to(s, CastKind::IntegralCast, e, p, None),
        None => e,
    }
}

/// The usual unary conversions applied to an arithmetic operand (clang's
/// `UsualUnaryConversions`): [`decay`], `__fp16` to `float`
/// (`FloatingCast`; [basic.extended.fp] storage-only type), then the
/// integral promotions.
pub fn usual_unary_conversions(s: &mut Sema<'_, '_>, e: ExprId) -> ExprId {
    let bw = bit_field_width(s, e);
    let e = decay(s, e);
    let t = ty(s, e);
    if s.ast.types.builtin(t) == Some(kiln_ast::types::BuiltinType::Half) {
        return cast_to(s, CastKind::FloatingCast, e, QualType::FLOAT, None);
    }
    promote_int(s, e, bw)
}

/// True if `e` is a null pointer constant ([conv.ptr]/1): an integer
/// literal with value zero or a prvalue of type `std::nullptr_t`
/// (through parentheses and `ConstantExpr`).
pub fn is_null_pointer_constant(s: &Sema<'_, '_>, e: ExprId) -> bool {
    if s.ast.types.builtin(ty(s, e)) == Some(kiln_ast::types::BuiltinType::NullPtr) {
        return true;
    }
    let mut cur = e;
    for _ in 0..256 {
        let n = s.ast.exprs.get(cur);
        match n.kind() {
            ExprKind::ParenExpr | ExprKind::ConstantExpr => cur = s.ast.exprs.operand(cur),
            ExprKind::IntegerLiteral => {
                return match s.ast.exprs.int_value(cur) {
                    Some(v) => v == 0,
                    None => false,
                };
            }
            _ => return false,
        }
    }
    false
}

/// clang's `{rvalue|lvalue}` select: an xvalue (such as an implicitly
/// moved `return` operand, [class.copy.elision]/3) prints as "rvalue".
fn category_select(s: &Sema<'_, '_>, e: ExprId) -> &'static str {
    if s.ast.exprs.get(e).value_category() == ValueCategory::LValue { "1" } else { "0" }
}

fn entity_select(ctx: ConvCtx) -> &'static str {
    match ctx {
        ConvCtx::Initialization | ConvCtx::DirectInitialization => "0",
        ConvCtx::Argument => "1",
        ConvCtx::Return => "2",
        _ => "3",
    }
}

pub(crate) fn has_conversion_functions(s: &Sema<'_, '_>, q: QualType) -> bool {
    let Some(d) = s.ast.types.as_record(q) else { return false };
    let Some(def) = bases::complete_class(s, d) else { return false };
    s.ast
        .decls
        .members(def)
        .any(|m| s.ast.decls.get(m).kind() == kiln_ast::decl::DeclKind::CXXConversion)
}

/// The "remove `*`" / "remove `&`" / "dereference" / "take the address"
/// hint clang appends to an assignment int↔pointer diagnostic.
fn pointer_fix_hint(
    s: &mut Sema<'_, '_>,
    e: ExprId,
    to: QualType,
    int_to_ptr: bool,
) -> &'static str {
    let from = ty(s, e);
    let n = *s.ast.exprs.get(e);
    if n.kind() == ExprKind::UnaryOperator {
        let op = s.ast.exprs.unary_op(e);
        let sub = s.ast.exprs.operand(e);
        let st = ty(s, sub);
        if int_to_ptr
            && op == kiln_ast::expr::UnaryOp::Deref
            && s.ast.types.same_unqualified(st, to)
        {
            return "3";
        }
        if !int_to_ptr
            && op == kiln_ast::expr::UnaryOp::AddrOf
            && s.ast.types.same_unqualified(st, to)
        {
            return "4";
        }
    }
    if int_to_ptr {
        if n.value_category() == ValueCategory::LValue {
            let p = s.ast.types.pointer_to(from.without_quals());
            if s.ast.types.same_unqualified(p, to) {
                return "2";
            }
        }
    } else if let Some(p) = s.ast.types.pointee(from)
        && s.ast.types.same_unqualified(p, to)
    {
        return "1";
    }
    "0"
}

/// The diagnostic for a failed conversion of `e` to `to` in `ctx`.
fn conversion_error(s: &mut Sema<'_, '_>, e: ExprId, to: QualType, ctx: ConvCtx) -> ConvError {
    let from = ty(s, e);
    let (fa, ta) = (s.ty_arg(from), s.ty_arg(to));
    let types = &s.ast.types;
    let record_src = types.as_record(from).is_some();
    match ctx {
        ConvCtx::Condition => {
            ConvError::new(DiagId::sema_not_contextually_convertible_to_bool, vec![fa])
        }
        ConvCtx::Assignment => {
            let from_d = {
                let n = *s.ast.exprs.get(e);
                if s.ast.types.is_array(from) && n.value_category().is_glvalue() {
                    array_decayed(s, from)
                } else if s.ast.types.is_function(from) {
                    s.ast.types.pointer_to(from)
                } else {
                    from
                }
            };
            let nullptr_src =
                s.ast.types.builtin(from_d) == Some(kiln_ast::types::BuiltinType::NullPtr);
            let to_ptr = s.ast.types.is_pointer(to);
            let from_ptr = s.ast.types.is_pointer(from_d);
            let from_int = is_integral_or_unscoped_enum(s, from_d);
            let to_int = is_integral_or_unscoped_enum(s, to);
            if to_ptr && from_int {
                let h = pointer_fix_hint(s, e, to, true);
                return ConvError::new(DiagId::sema_assign_int_to_pointer, vec![ta, fa, h.into()]);
            }
            if to_int && (from_ptr || nullptr_src) {
                let h = pointer_fix_hint(s, e, to, false);
                return ConvError::new(DiagId::sema_assign_pointer_to_int, vec![ta, fa, h.into()]);
            }
            if to_ptr && from_ptr {
                return match qualification_relation(s, from_d, to) {
                    QualRelation::Similar { drops_outer: true, .. } => {
                        ConvError::new(DiagId::sema_assign_discards_qualifiers, vec![ta, fa])
                    }
                    QualRelation::Similar { drops_nested: true, .. } => {
                        ConvError::new(DiagId::sema_assign_discards_nested_qualifiers, vec![ta, fa])
                    }
                    _ => ConvError::new(DiagId::sema_assign_incompatible_pointer, vec![ta, fa]),
                };
            }
            ConvError::new(DiagId::sema_assign_incompatible, vec![ta, fa])
        }
        _ if record_src => {
            if ctx == ConvCtx::Return {
                ConvError::new(DiagId::sema_no_viable_conversion_return, vec![fa, ta])
            } else {
                ConvError::new(DiagId::sema_no_viable_conversion, vec![fa, ta])
            }
        }
        _ => {
            let suffix = s.fn_mismatch_suffix(from, to);
            ConvError::new(
                DiagId::sema_init_conversion_failed,
                vec![entity_select(ctx).into(), ta, category_select(s, e).into(), fa, suffix],
            )
        }
    }
}

fn unsupported(what: &str) -> ConvError {
    ConvError::new(DiagId::sema_conv_unsupported, vec![what.into()])
}

/// Binds a reference of type `to` to the glvalue `e` ([dcl.init.ref]/5.1,
/// 5.3 for reference-compatible glvalues). Temporaries and conversion
/// functions are not supported yet (M3).
fn bind_reference(s: &mut Sema<'_, '_>, e: ExprId, to: QualType) -> Result<ExprId, ConvError> {
    let (rvalue_ref, referee) = match s.ast.types.canonical_view(to) {
        TypeView::LValueRef(p) => (false, p),
        TypeView::RValueRef(p) => (true, p),
        _ => return Err(unsupported("this reference binding")),
    };
    let n = *s.ast.exprs.get(e);
    let from = n.ty();
    let vk = n.value_category();
    let (rq, fq) = (s.ast.types.effective_quals(referee), s.ast.types.effective_quals(from));
    let f_un = s.ast.types.canonical(from).without_quals();
    let r_un = s.ast.types.canonical(referee).without_quals();
    // Reference-related: same type, or `referee` a base of `from`
    // ([dcl.init.ref]/4).
    let same = s.ast.types.same_type(f_un, r_un);
    let base = match (s.ast.types.as_record(f_un), s.ast.types.as_record(r_un)) {
        (Some(d), Some(b)) if !same => find_base(s, d, b),
        _ => BaseSearch::NotDerived,
    };
    let related = same || base.is_derived();
    let to_str = s.ty_arg(referee);
    let bind_ok_category =
        if rvalue_ref { vk == ValueCategory::XValue } else { vk == ValueCategory::LValue };
    // A non-const (or volatile) lvalue reference: binds no temporary.
    let plain_lref = !rvalue_ref && (!rq.contains(Quals::CONST) || rq.contains(Quals::VOLATILE));
    if !related {
        if plain_lref {
            let sel = if rq.contains(Quals::VOLATILE) { "1" } else { "0" };
            let id = if vk == ValueCategory::LValue {
                DiagId::sema_lvalue_ref_bind_to_unrelated
            } else {
                DiagId::sema_lvalue_ref_bind_to_temporary
            };
            return Err(ConvError::new(id, vec![sel.into(), to_str, s.ty_arg(from)]));
        }
        return Err(unsupported("binding a reference to a temporary"));
    }
    if !rq.contains(fq) {
        let dropped = fq - rq;
        let names: Vec<&str> =
            [(Quals::CONST, "const"), (Quals::VOLATILE, "volatile"), (Quals::RESTRICT, "restrict")]
                .iter()
                .filter(|(q, _)| dropped.contains(*q))
                .map(|&(_, n)| n)
                .collect();
        let plural = if names.len() > 1 { "1" } else { "0" };
        let quoted = format!("'{}'", names.join(" "));
        return Err(ConvError::new(
            DiagId::sema_reference_bind_drops_quals,
            vec![to_str, s.ty_arg(from), quoted, plural.into()],
        ));
    }
    if !bind_ok_category {
        if rvalue_ref && vk == ValueCategory::LValue {
            return Err(ConvError::new(
                DiagId::sema_rvalue_ref_bind_to_lvalue,
                vec![to_str, s.ty_arg(from)],
            ));
        }
        if plain_lref && vk == ValueCategory::PRValue {
            let sel = if rq.contains(Quals::VOLATILE) { "1" } else { "0" };
            return Err(ConvError::new(
                DiagId::sema_lvalue_ref_bind_to_temporary,
                vec![sel.into(), to_str, s.ty_arg(from)],
            ));
        }
        return Err(unsupported("binding a reference to a temporary"));
    }
    let out_vk = vk;
    let target = strip_cv(s, referee);
    let target = s.ast.types.qualify(target, rq);
    match base {
        BaseSearch::NotDerived => {
            if s.ast.types.canonical(from) == s.ast.types.canonical(target) {
                Ok(e)
            } else {
                Ok(raw_cast(s, CastKind::NoOp, e, target, out_vk, None))
            }
        }
        BaseSearch::Unique(p) => {
            // [dcl.init.ref]/5: the base must be accessible (diagnosed;
            // the binding still applies).
            let at = s.begin_loc(e);
            let (d, b) = (strip_cv(s, from), strip_cv(s, referee));
            s.check_base_access(d, b, &p, at);
            let list = path_list(s, &p);
            Ok(raw_cast(s, CastKind::DerivedToBase, e, target, out_vk, Some(list)))
        }
        BaseSearch::Ambiguous(paths) => {
            let shown = bases::ambiguous_paths_string(s, from, &paths);
            Err(ConvError::new(
                DiagId::sema_ambiguous_derived_to_base,
                vec![s.ty_arg(strip_cv(s, from)), s.ty_arg(strip_cv(s, referee)), shown],
            ))
        }
    }
}

/// Converts `e` to type `to` in context `ctx` by an implicit conversion
/// sequence ([over.best.ics]): a standard conversion sequence for
/// non-class types ([over.ics.scs]), or a direct reference binding to a
/// reference-compatible glvalue ([dcl.init.ref]). User-defined
/// conversions are not implemented yet (M3): a class source without
/// conversion functions gets clang's "no viable conversion", others
/// `sema_conv_unsupported`.
///
/// # Errors
/// If no conversion exists; the error is clang's diagnostic for `ctx`.
pub fn implicit_convert(
    s: &mut Sema<'_, '_>,
    e: ExprId,
    to: QualType,
    ctx: ConvCtx,
) -> Result<ExprId, ConvError> {
    let from = ty(s, e);
    if is_error(s, from) || is_error(s, to) {
        return Ok(e);
    }
    if s.ast.types.is_dependent(from) || s.ast.types.is_dependent(to) {
        return Ok(e);
    }
    if s.check_builtin_fn_use(e) {
        let loc = s.eloc(e);
        return Ok(s.recover_err(&[e], loc));
    }
    if s.ast.types.is_reference(to) {
        return bind_reference(s, e, to);
    }
    let (fr, tr) = (s.ast.types.as_record(from), s.ast.types.as_record(to));
    if tr.is_some() {
        let prvalue = s.ast.exprs.get(e).value_category() == ValueCategory::PRValue;
        if prvalue && s.ast.types.same_unqualified(from, to) {
            return Ok(e);
        }
        if fr.is_none() && !has_constructors_or_unknown(s, to) {
            return Err(conversion_error(s, e, to, ctx));
        }
        return Err(unsupported("initialization of a class object"));
    }
    if fr.is_some() {
        if has_conversion_functions(s, from) {
            return Err(unsupported("a user-defined conversion"));
        }
        return Err(conversion_error(s, e, to, ctx));
    }
    match standard_conversion(s, e, to, ctx) {
        Some(scs) => apply_standard_conversion(s, e, &scs, to),
        None => Err(conversion_error(s, e, to, ctx)),
    }
}

/// Conservatively true: class initialization is M3 work.
fn has_constructors_or_unknown(_s: &Sema<'_, '_>, _to: QualType) -> bool {
    true
}

/// The usual arithmetic conversions ([expr.arith.conv]) for a binary
/// arithmetic operator: [`usual_arithmetic_conversions_kind`] with
/// [`ArithConvKind::Arithmetic`].
///
/// # Errors
/// As for [`usual_arithmetic_conversions_kind`].
pub fn usual_arithmetic_conversions(
    s: &mut Sema<'_, '_>,
    l: ExprId,
    r: ExprId,
) -> Result<(ExprId, ExprId, QualType), ConvError> {
    usual_arithmetic_conversions_kind(s, l, r, ArithConvKind::Arithmetic)
}

/// Contextually converts `e` to `bool` ([conv.general]/4): `bool t(e);`.
///
/// # Errors
/// `sema_not_contextually_convertible_to_bool` if there is no such
/// conversion.
pub fn contextually_convert_to_bool(s: &mut Sema<'_, '_>, e: ExprId) -> Result<ExprId, ConvError> {
    implicit_convert(s, e, QualType::BOOL, ConvCtx::Condition)
}
