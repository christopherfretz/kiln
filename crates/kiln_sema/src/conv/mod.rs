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
pub(crate) use qual::{
    CastAway, casts_away_constness, cv_combined_type, drop_noexcept, function_ref_compatible,
    is_function_conversion, similar_ref_relation, similar_same_levels,
};
pub(crate) mod rank;
pub(crate) mod scs;
pub(crate) mod soft;

use kiln_ast::expr::{CastKind, ExprKind, ObjectKind, ValueCategory};
use kiln_ast::types::{Quals, TypeView};
use kiln_ast::{ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::{Loc, Span};

use crate::Sema;
use crate::overload::ics::IcsOptions;

pub use apply::apply_standard_conversion;
pub(crate) use apply::fp_rank_check;
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
    ConvRank, FirstStep, FixedEnumPromotion, IcsCompare, ListArray, RefBinding, SecondStep,
    StandardConversion, ThirdStep, compare_standard_conversions, standard_conversion,
    standard_conversion_from,
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
#[derive(Clone, Debug)]
pub struct ConvError {
    /// The diagnostic.
    pub id: DiagId,
    /// Its arguments, in order (type arguments pre-quoted, as
    /// [`Sema::ty_arg`] prints them).
    pub args: Vec<String>,
    /// Its notes (the candidates of a failed user-defined conversion).
    pub notes: Vec<kiln_diag::Diag>,
    /// Where the diagnostic goes regardless of the caller's choice (a
    /// conversion step clang diagnoses at the converted expression).
    pub at: Option<Loc>,
    /// A source range underlined with `~` (clang streams the
    /// initializer's range into the reference-binding diagnostics).
    pub range: Option<Span>,
}

impl ConvError {
    pub(crate) fn new(id: DiagId, args: Vec<String>) -> ConvError {
        ConvError { id, args, notes: Vec::new(), at: None, range: None }
    }

    /// The diagnostic, with its caret at `e`'s location.
    pub fn diag(&self, s: &Sema<'_, '_>, e: ExprId) -> kiln_diag::Diag {
        self.diag_at(s, s.ast.exprs.get(e).loc())
    }

    /// The diagnostic, with its caret at `loc`.
    pub fn diag_at(&self, s: &Sema<'_, '_>, loc: Loc) -> kiln_diag::Diag {
        let entity = s.init_entity.filter(|_| self.id == DiagId::sema_init_conversion_failed);
        let loc = self.at.unwrap_or(loc);
        let d = self.args.iter().enumerate().fold(s.diag(self.id, loc), |d, (i, a)| match entity {
            Some(sel) if i == 0 && a == "0" => d.arg(sel),
            _ => d.arg(a.as_str()),
        });
        let d = match self.range {
            Some(r) => d.range(r),
            None => d,
        };
        self.notes.iter().cloned().fold(d, kiln_diag::Diag::with_note)
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
    s.check_null_indirection(e);
    let types = &s.ast.types;
    let to = strip_cv(s, t);
    let ck = if types.builtin(t) == Some(kiln_ast::types::BuiltinType::NullPtr) {
        CastKind::NullToPointer
    } else {
        CastKind::LValueToRValue
    };
    let atomic = types.as_atomic(t);
    let r = raw_cast(s, ck, e, to, ValueCategory::PRValue, None);
    // clang's `DefaultLvalueConversion`: the load of an `_Atomic(T)` is
    // then converted to the unqualified `T` (C11 6.3.2.1p2).
    match atomic {
        Some(v) => {
            let v = strip_cv(s, v);
            raw_cast(s, CastKind::AtomicToNonAtomic, r, v, ValueCategory::PRValue, None)
        }
        None => r,
    }
}

/// The array-to-pointer ([conv.array]) and function-to-pointer
/// ([conv.func]) conversions, else the lvalue-to-rvalue conversion
/// (clang's `DefaultFunctionArrayLvalueConversion`).
pub fn decay(s: &mut Sema<'_, '_>, e: ExprId) -> ExprId {
    let e = s.resolve_single_overload_candidate(e);
    let n = *s.ast.exprs.get(e);
    let t = n.ty();
    if s.ast.types.is_dependent(t) {
        return e;
    }
    // [expr.ref]/7.3.2: recovered by the zero-argument call, as clang's
    // `CheckPlaceholderExpr` does (WP-832).
    if t == QualType::BOUND_MEMBER {
        let r = s.recover_not_called(e);
        return if s.is_err_expr(r) { r } else { decay(s, r) };
    }
    if s.check_builtin_fn_use(e) || s.check_overload_use(e) {
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
        // An array prvalue (a compound literal) is materialized first
        // ([conv.array]/1, clang `DefaultFunctionArrayConversion`).
        let e =
            if n.value_category().is_glvalue() { e } else { s.materialize_temporary(e, t, false) };
        let p = array_decayed(s, t);
        return raw_cast(s, CastKind::ArrayToPointerDecay, e, p, ValueCategory::PRValue, None);
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
pub(crate) fn eval_width(s: &Sema<'_, '_>, w: ExprId) -> Option<u32> {
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
    let f = source_bit_field(s, e)?;
    let w = s.ast.decls.field(f)?.bit_width?;
    eval_width(s, w)
}

/// The bit-field `e` designates, if any: through parentheses, no-op and
/// lvalue-to-rvalue casts and a structured binding's expression (clang
/// `Expr::getSourceBitField`).
pub fn source_bit_field(s: &Sema<'_, '_>, e: ExprId) -> Option<kiln_ast::DeclId> {
    let mut cur = e;
    for _ in 0..256 {
        let n = s.ast.exprs.get(cur);
        match n.kind() {
            ExprKind::ParenExpr => cur = s.ast.exprs.operand(cur),
            // clang `IgnoreParens`: a resolved `__builtin_choose_expr` or
            // `_Generic` is its chosen operand.
            ExprKind::ChooseExpr | ExprKind::GenericSelectionExpr => {
                cur = s.ast.exprs.selected_operand(cur)?;
            }
            ExprKind::ImplicitCastExpr
                if matches!(
                    s.ast.exprs.cast_kind(cur),
                    CastKind::NoOp | CastKind::LValueToRValue
                ) =>
            {
                cur = s.ast.exprs.operand(cur);
            }
            ExprKind::MemberExpr if n.object_kind() == ObjectKind::BitField => {
                return Some(s.ast.exprs.decl(cur));
            }
            ExprKind::DeclRefExpr if n.object_kind() == ObjectKind::BitField => {
                cur = s.ast.decls.binding_expr(s.ast.exprs.decl(cur))?;
            }
            // [expr.assign]/3, [expr.pre.incr]/1, [expr.comma]/1: an
            // assignment, a prefix increment or decrement and a comma whose
            // right operand is one designate that bit-field, so their
            // result promotes as it does ([conv.prom]/5).
            ExprKind::CompoundAssignOperator => cur = s.ast.exprs.children(cur, &s.ast.lists)[0],
            ExprKind::BinaryOperator => {
                let [l, r] = s.ast.exprs.operands(cur);
                match s.ast.exprs.binary_op(cur) {
                    op if op.is_assignment() => cur = l,
                    kiln_ast::expr::BinaryOp::Comma => cur = r,
                    _ => return None,
                }
            }
            ExprKind::UnaryOperator
                if matches!(
                    s.ast.exprs.unary_op(cur),
                    kiln_ast::expr::UnaryOp::PreInc | kiln_ast::expr::UnaryOp::PreDec
                ) =>
            {
                cur = s.ast.exprs.operand(cur);
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
/// literal with value zero, GNU `__null`, or a prvalue of type
/// `std::nullptr_t` (through parentheses, `ConstantExpr` and a resolved
/// `__builtin_choose_expr` or `_Generic`).
pub fn is_null_pointer_constant(s: &Sema<'_, '_>, e: ExprId) -> bool {
    if s.ast.types.builtin(ty(s, e)) == Some(kiln_ast::types::BuiltinType::NullPtr) {
        return true;
    }
    let mut cur = e;
    for _ in 0..256 {
        let n = s.ast.exprs.get(cur);
        match n.kind() {
            ExprKind::ParenExpr | ExprKind::ConstantExpr => cur = s.ast.exprs.operand(cur),
            ExprKind::ChooseExpr | ExprKind::GenericSelectionExpr => {
                match s.ast.exprs.selected_operand(cur) {
                    Some(c) => cur = c,
                    None => return false,
                }
            }
            ExprKind::GNUNullExpr => return true,
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

/// Whether class type `q` has a visible conversion function: its own,
/// a base's ([class.conv.fct]/9: conversion functions are inherited), or
/// one a using-declaration names ([namespace.udecl]/1), the set
/// [`Sema::conversion_functions_all`] collects.
pub(crate) fn has_conversion_functions(s: &Sema<'_, '_>, q: QualType) -> bool {
    let Some(d) = s.ast.types.as_record(q) else { return false };
    if bases::complete_class(s, d).is_none() {
        return false;
    }
    !s.conversion_functions_all(q).is_empty()
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
    // clang's `%diff` pair (spec/diag.md §6.4).
    let (fa, ta) = s.ty_diff(from, to);
    let types = &s.ast.types;
    let record_src = types.as_record(from).is_some();
    match ctx {
        ConvCtx::Condition => {
            ConvError::new(DiagId::sema_not_contextually_convertible_to_bool, vec![s.ty_arg(from)])
        }
        // A function template-id resolved early is an overload set to
        // clang ([`Sema::diagnose_overload_assign`]).
        ConvCtx::Assignment if let Some(ule) = s.overload_set_of(e) => {
            let notes = s.overload_assign_notes(ule, to);
            let mut err = ConvError::new(
                DiagId::sema_assign_incompatible,
                vec![s.ty_arg(to), s.ty_arg(QualType::OVERLOAD)],
            );
            err.notes = notes;
            err
        }
        // [over.over]/1: a function template-id resolved early is an
        // overload set to clang, finding no match for the target.
        _ if let Some(err) = template_id_no_match(s, e, to, ctx) => err,
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

/// [over.over]/1: the initialization (or `return`) of `to` by `e`, a
/// function template-id resolved early whose specialization does not
/// convert, finds no match ([`Sema::template_id_no_match`]).
fn template_id_no_match(
    s: &mut Sema<'_, '_>,
    e: ExprId,
    to: QualType,
    ctx: ConvCtx,
) -> Option<ConvError> {
    if !matches!(ctx, ConvCtx::Initialization | ConvCtx::DirectInitialization | ConvCtx::Return) {
        return None;
    }
    let (at, name, fnty, notes) = s.template_id_no_match(e, to)?;
    let mut err = ConvError::new(DiagId::sema_addr_ovl_no_match, vec![name, s.ty_arg(fnty)]);
    err.notes = notes;
    err.at = Some(at);
    Some(err)
}

/// [over.over]/1: the initialization (or `return`) of `to` by `e`, a
/// function template-id resolved early whose specialization does not
/// convert, re-resolved against `to` ([`Sema::retarget_template_id`]).
fn retarget_template_id(
    s: &mut Sema<'_, '_>,
    e: ExprId,
    to: QualType,
    ctx: ConvCtx,
) -> Option<ExprId> {
    if !matches!(ctx, ConvCtx::Initialization | ConvCtx::DirectInitialization | ConvCtx::Return) {
        return None;
    }
    s.retarget_template_id(e, to)
}

fn unsupported(what: &str) -> ConvError {
    ConvError::new(DiagId::sema_conv_unsupported, vec![what.into()])
}

/// clang `FK_ReferenceInitFailed` for binding a reference to `referee`
/// (as written) to `e` of type `from`: "reference to {type|incomplete
/// type} T1 could not bind to an {rvalue|lvalue} of type T2", where an
/// xvalue is an rvalue (clang `isLValue`).
fn reference_bind_failed(
    s: &mut Sema<'_, '_>,
    referee: QualType,
    from: QualType,
    vk: ValueCategory,
) -> ConvError {
    // A function type is not incomplete (clang `isIncompleteType`).
    let incomplete = !s.ast.types.is_function(referee) && !s.is_complete(referee);
    let incomplete = if incomplete { "1" } else { "0" };
    let sel = if vk == ValueCategory::LValue { "1" } else { "0" };
    let (to_str, from_str) = s.ty_diff(referee, from);
    ConvError::new(
        DiagId::sema_reference_bind_failed,
        vec![to_str, incomplete.into(), sel.into(), from_str],
    )
}

/// Binds a reference of type `to` to the glvalue `e` ([dcl.init.ref]/5.1,
/// 5.3 for reference-compatible glvalues, /5.4 through a temporary).
#[allow(clippy::too_many_lines, reason = "one step per bullet of [dcl.init.ref]/5")]
fn bind_reference(
    s: &mut Sema<'_, '_>,
    e: ExprId,
    to: QualType,
    ctx: ConvCtx,
) -> Result<ExprId, ConvError> {
    let (rvalue_ref, referee) = match s.ast.types.canonical_view(to) {
        TypeView::LValueRef(p) => (false, p),
        TypeView::RValueRef(p) => (true, p),
        _ => return Err(unsupported("this reference binding")),
    };
    let n = *s.ast.exprs.get(e);
    let from = n.ty();
    let vk = n.value_category();
    let (rq, fq) = (s.ast.types.effective_quals(referee), s.ast.types.effective_quals(from));
    // Unqualified through arrays, so `const int[3]` is related to
    // `int[3]`.
    let unqual = |s: &mut Sema<'_, '_>, q: QualType| {
        let c = s.ast.types.canonical(q);
        let u = s.ast.types.unqualified(c);
        s.ast.types.canonical(u)
    };
    let f_un = unqual(s, from);
    let r_un = unqual(s, referee);
    // Reference-related: same type, or `referee` a base of `from`
    // ([dcl.init.ref]/4).
    // A `T[N]` glvalue binds a `T (&)[]` (P0388R4); clang treats the
    // pair as unrelated when the binding would drop qualifiers.
    let same = s.ast.types.same_type(f_un, r_un)
        || (rq.contains(fq) && s.unknown_bound_of(r_un, f_un))
        || function_ref_compatible(s, r_un, f_un);
    let base = match (s.ast.types.as_record(f_un), s.ast.types.as_record(r_un)) {
        (Some(d), Some(b)) if !same => {
            // The relation needs the initializer's class complete: a class
            // template specialization is instantiated here ([temp.inst]/2;
            // clang `isCompleteType(Loc, T2)` in
            // `CompareReferenceRelationship`), so `S<int>& r = d;` binds a
            // `D<int>` nothing else has completed.
            let at = s.begin_loc(e);
            let _ = s.require_complete_silent(f_un, at);
            find_base(s, d, b)
        }
        _ => BaseSearch::NotDerived,
    };
    // CWG2352 ([dcl.init.ref]/4): similar types are reference-related,
    // and reference-compatible when a pointer to the initializer's type
    // converts to a pointer to the referee by a qualification conversion:
    // `const int *const &r = p` binds the `int *` lvalue `p` directly.
    let similar_rel = if same || base.is_derived() || s.ast.types.is_function(r_un) {
        None
    } else {
        similar_ref_relation(s, referee, from)
    };
    // Similar, related, but not reference-compatible (`const int *&` and
    // `int *`).
    let incompatible_similar = similar_rel == Some(false);
    let related = same || base.is_derived() || similar_rel.is_some();
    // Diagnostics print the referee as written (clang
    // `DestType.getNonReferenceType()`): `I` in `I &&`.
    let written = s.ast.types.desugar_top(to);
    let to_ty = match s.ast.types.view(written.ty()) {
        TypeView::LValueRef(p) | TypeView::RValueRef(p) => p,
        _ => referee,
    };
    // The reference diagnostics format (to, from) as clang's `%diff`
    // The reference diagnostics format (to, from) as clang's `%diff`
    // pair (spec/diag.md §6.4), formatted only on failure.
    let strs = |s: &Sema<'_, '_>| s.ty_diff(to_ty, from);
    // A non-const (or volatile) lvalue reference: binds no temporary.
    let plain_lref = !rvalue_ref && (!rq.contains(Quals::CONST) || rq.contains(Quals::VOLATILE));
    // [dcl.init.ref]/5.1 binds an lvalue; /5.3.1 binds an xvalue (or a
    // function lvalue) to an rvalue reference or a const lvalue
    // reference directly.
    let bind_ok_category = match vk {
        ValueCategory::LValue => !rvalue_ref || s.ast.types.is_function(r_un),
        ValueCategory::XValue => !plain_lref,
        ValueCategory::PRValue => false,
    };
    if !plain_lref
        && vk == ValueCategory::PRValue
        && related
        && !incompatible_similar
        && rq.contains(fq)
    {
        // [dcl.init.ref]/5.3.1: a reference-compatible prvalue is
        // materialized and the reference binds the temporary.
        return bind_to_temporary(s, e, referee, rq, rvalue_ref, base);
    }
    if !plain_lref
        && (!related || incompatible_similar)
        && s.ast.types.as_record(f_un).is_none()
        && s.ast.types.as_record(r_un).is_none()
        && !s.ast.types.is_function(r_un)
        && !s.ast.types.is_array(r_un)
    {
        // [dcl.init.ref]/5.4.2: a temporary of type cv1 T1
        // copy-initialized from the initializer.
        let target = s.ast.types.qualify(r_un, rq);
        // clang `FK_ReferenceInitFailed`: the referee as written.
        let Some(scs) = standard_conversion(s, e, r_un, ConvCtx::Initialization) else {
            return Err(reference_bind_failed(s, to_ty, from, vk));
        };
        if incompatible_similar {
            // /5.4.3, /5.4.4: a related initializer keeps its top-level
            // qualifiers, and an rvalue reference binds no lvalue (clang
            // checks both after forming the temporary's conversion).
            if !rq.contains(fq) {
                return Err(drops_quals_error(fq - rq, strs(s), s.expr_range(e)));
            }
            if rvalue_ref && vk == ValueCategory::LValue {
                let (to_str, from_str) = strs(s);
                return Err(ConvError::new(
                    DiagId::sema_rvalue_ref_bind_to_lvalue,
                    vec![to_str, from_str],
                ));
            }
        }
        let mut x = apply_standard_conversion(s, e, &scs, r_un)?;
        if !rq.is_empty() {
            x = qualify_prvalue(s, x, target);
        }
        let x = s.materialize_temporary(x, target, !rvalue_ref);
        return Ok(x);
    }
    if !related
        && s.ast.types.is_function(r_un)
        && let Some(x) = retarget_template_id(s, e, to, ctx)
    {
        return bind_reference(s, x, to, ctx);
    }
    if !related
        && s.ast.types.is_function(r_un)
        && let Some(err) = template_id_no_match(s, e, to, ctx)
    {
        return Err(err);
    }
    if !related {
        // [dcl.init.ref]/5.1.2, 5.3.2: a conversion function's result;
        // /5.4.1: a temporary initialized by a user-defined conversion.
        let class_src = s.ast.types.as_record(f_un).is_some();
        let class_ref = s.ast.types.as_record(r_un).is_some();
        if class_src || (class_ref && !plain_lref) {
            let at = s.begin_loc(e);
            // [over.match.ref]/1.1: a direct-initialization considers the
            // explicit conversion functions (not the explicit constructors
            // of a /5.4.1 temporary).
            let opts =
                if ctx.is_direct() { IcsOptions::EXPLICIT_REF_CONV } else { IcsOptions::NONE };
            if let Some(r) = s.user_convert_with(e, to, ConvCtx::Initialization, opts, at) {
                // clang: the /5.4.1 temporary's ambiguous implicit
                // conversion sequence, reported where it is performed.
                if !class_src
                    && let Err(err) = &r
                    && err.id == DiagId::sema_ref_init_ambiguous
                {
                    let (to_str, from_str) = strs(s);
                    let mut d =
                        s.diag(DiagId::sema_ovl_ambiguous_conversion, at).arg(from_str).arg(to_str);
                    for n in &err.notes {
                        d = d.with_note(n.clone());
                    }
                    s.emit(d);
                    return Ok(s.recover_err(&[e], at));
                }
                // [class.abstract]/3: the /5.4.1 temporary is an object
                // of an abstract class (clang's `checkAbstractType`).
                if !class_src
                    && let Ok(x) = r
                    && !s.is_err_expr(x)
                    && s.require_non_abstract_alloc(at, referee)
                {
                    return Ok(s.recover_err(&[x], at));
                }
                return r;
            }
            if !class_src && !plain_lref {
                // clang `FK_ReferenceInitFailed`: no implicit conversion
                // sequence to the /5.4.1 temporary (no conversion
                // function set to report).
                return Err(reference_bind_failed(s, to_ty, from, vk));
            }
            let lref_opts =
                if ctx.is_direct() { IcsOptions::ALLOW_EXPLICIT } else { IcsOptions::NONE };
            if !plain_lref || (class_src && s.has_lvalue_ref_conversion_candidates(from, lref_opts))
            {
                let mut err = ConvError::new(DiagId::sema_no_viable_conversion, {
                    let (f, r) = s.ty_diff(from, referee);
                    vec![f, r]
                });
                err.notes = s.udc_failure_notes_with(e, to, opts);
                return Err(err);
            }
        }
        if plain_lref {
            let sel = if rq.contains(Quals::VOLATILE) { "1" } else { "0" };
            let id = if vk == ValueCategory::LValue {
                DiagId::sema_lvalue_ref_bind_to_unrelated
            } else {
                DiagId::sema_lvalue_ref_bind_to_temporary
            };
            let (to_str, from_str) = strs(s);
            return Err(ConvError::new(id, vec![sel.into(), to_str, from_str]));
        }
        // No temporary of array type is copy-initialized from an
        // unrelated array, and no temporary of function type exists
        // ([dcl.init.ref]/5.4.2; clang `FK_ReferenceInitFailed`).
        return Err(reference_bind_failed(s, to_ty, from, vk));
    }
    if related && plain_lref && vk != ValueCategory::LValue {
        // [dcl.init.ref]/5.2: a non-const lvalue reference binds only an
        // lvalue; an rvalue, including an xvalue (`static_cast<T&&>(x)`,
        // or a returned local under P2266's implicit move,
        // [expr.prim.id.unqual]/3), is diagnosed as a temporary even when
        // the binding would also drop qualifiers or name a bit-field (clang
        // `FK_NonConstLValueReferenceBindingToTemporary`).
        let sel = if rq.contains(Quals::VOLATILE) { "1" } else { "0" };
        let (to_str, from_str) = strs(s);
        return Err(ConvError::new(
            DiagId::sema_lvalue_ref_bind_to_temporary,
            vec![sel.into(), to_str, from_str],
        ));
    }
    if related
        && vk.is_glvalue()
        && s.ast.exprs.get(e).object_kind() == ObjectKind::VectorComponent
        && plain_lref
    {
        // A vector element is not referenceable either (clang
        // `err_reference_bind_to_vector_element`).
        let sel = if rq.contains(Quals::VOLATILE) { "1" } else { "0" };
        return Err(ConvError::new(
            DiagId::sema_reference_bind_to_vector_element,
            vec![sel.into()],
        ));
    }
    if related
        && vk.is_glvalue()
        && matches!(
            s.ast.exprs.get(e).object_kind(),
            ObjectKind::BitField | ObjectKind::VectorComponent
        )
    {
        // A bit-field is not referenceable ([dcl.init.ref]/5.1, clang
        // `isNonReferenceableGLValue`): a non-const lvalue reference is
        // ill-formed, any other binds a temporary copy (/5.4.2).
        if plain_lref {
            let sel = if rq.contains(Quals::VOLATILE) { "1" } else { "0" };
            let f = source_bit_field(s, e);
            let name = f.map_or_else(String::new, |f| format!(" '{}'", s.decl_name(f)));
            let mut err =
                ConvError::new(DiagId::sema_reference_bind_to_bitfield, vec![sel.into(), name]);
            if let Some(f) = f {
                let at = s.ast.decls.get(f).loc();
                err.notes.push(s.diag(DiagId::note_bitfield_decl, at));
            }
            return Err(err);
        }
        let target = s.ast.types.qualify(r_un, rq);
        let scs = standard_conversion(s, e, r_un, ConvCtx::Initialization)
            .ok_or_else(|| conversion_error(s, e, r_un, ConvCtx::Initialization))?;
        // clang materializes the cv1 T1 temporary from the unqualified
        // value, without a qualification cast.
        let x = apply_standard_conversion(s, e, &scs, r_un)?;
        return Ok(s.materialize_temporary(x, target, !rvalue_ref));
    }
    if !rq.contains(fq) {
        return Err(drops_quals_error(fq - rq, strs(s), s.expr_range(e)));
    }
    if incompatible_similar {
        // A non-const lvalue reference to a similar lvalue whose nested
        // qualifiers do not convert (clang `FK_ReferenceInitDropsQualifiers`
        // with no top-level qualifier dropped).
        let (to_str, from_str) = strs(s);
        return Err(ConvError::new(
            DiagId::sema_reference_bind_incompatible_quals,
            vec![to_str, from_str],
        ));
    }
    if !bind_ok_category {
        // The remaining case: an rvalue reference and an lvalue (a
        // non-const lvalue reference and an rvalue was diagnosed above).
        debug_assert!(rvalue_ref && vk == ValueCategory::LValue, "bind category");
        let (to_str, from_str) = strs(s);
        return Err(ConvError::new(DiagId::sema_rvalue_ref_bind_to_lvalue, vec![to_str, from_str]));
    }
    // A direct binding (clang `SK_BindReference`).
    s.check_null_reference_binding(e);
    let out_vk = vk;
    let target = strip_cv(s, referee);
    let target = s.ast.types.qualify(target, rq);
    match base {
        BaseSearch::NotDerived => {
            if s.ast.types.canonical(from) == s.ast.types.canonical(target) {
                Ok(e)
            } else if n.kind() == ExprKind::ImplicitCastExpr
                && s.ast.exprs.cast_kind(e) == CastKind::NoOp
            {
                // clang adjusts the qualifiers in the cast that made the
                // glvalue (the implicit move of [class.copy.elision]/3).
                let inner = s.ast.exprs.operand(e);
                Ok(raw_cast(s, CastKind::NoOp, inner, target, out_vk, None))
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
            // clang first adds the qualifiers to the derived glvalue.
            let x = if rq == fq {
                e
            } else {
                let qd = s.ast.types.qualify(d, rq);
                raw_cast(s, CastKind::NoOp, e, qd, out_vk, None)
            };
            Ok(raw_cast(s, CastKind::DerivedToBase, x, target, out_vk, Some(list)))
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

/// "binding reference of type T1 to value of type T2 drops Q
/// qualifier(s)" for the top-level qualifiers `dropped`, with the
/// (to, from) type strings `strs`, underlining the initializer's
/// `range` as clang does.
fn drops_quals_error(dropped: Quals, strs: (String, String), range: Span) -> ConvError {
    let names: Vec<&str> =
        [(Quals::CONST, "const"), (Quals::VOLATILE, "volatile"), (Quals::RESTRICT, "restrict")]
            .iter()
            .filter(|(q, _)| dropped.contains(*q))
            .map(|&(_, n)| n)
            .collect();
    let plural = if names.len() > 1 { "1" } else { "0" };
    let quoted = format!("'{}'", names.join(" "));
    let (to_str, from_str) = strs;
    ConvError {
        range: Some(range),
        ..ConvError::new(
            DiagId::sema_reference_bind_drops_quals,
            vec![to_str, from_str, quoted, plural.into()],
        )
    }
}

/// Binds a reference to the temporary materialized from the
/// reference-compatible prvalue `e` ([dcl.init.ref]/5.3.1, clang
/// `PerformInitialization`'s `SK_BindReferenceToTemporary`): `e` is
/// adjusted to cv1 T2, materialized (an lvalue for an lvalue
/// reference), then converted to the base T1.
fn bind_to_temporary(
    s: &mut Sema<'_, '_>,
    e: ExprId,
    referee: QualType,
    rq: Quals,
    rvalue_ref: bool,
    base: BaseSearch,
) -> Result<ExprId, ConvError> {
    let from = ty(s, e);
    let src = strip_cv(s, from);
    let qualified = s.ast.types.qualify(src, rq);
    let x = qualify_prvalue(s, e, qualified);
    let x = s.materialize_temporary(x, qualified, !rvalue_ref);
    let vk = if rvalue_ref { ValueCategory::XValue } else { ValueCategory::LValue };
    match base {
        BaseSearch::NotDerived => Ok(x),
        BaseSearch::Unique(p) => {
            let at = s.begin_loc(e);
            let b = strip_cv(s, referee);
            s.check_base_access(src, b, &p, at);
            let list = path_list(s, &p);
            let target = s.ast.types.qualify(b, rq);
            Ok(raw_cast(s, CastKind::DerivedToBase, x, target, vk, Some(list)))
        }
        BaseSearch::Ambiguous(paths) => {
            let shown = bases::ambiguous_paths_string(s, from, &paths);
            Err(ConvError::new(
                DiagId::sema_ambiguous_derived_to_base,
                vec![s.ty_arg(src), s.ty_arg(strip_cv(s, referee)), shown],
            ))
        }
    }
}

/// The prvalue `e` with the qualifiers of `to` (its own type plus cv): a
/// `NoOp` cast when they differ, retyping the cast `e` itself when it is
/// an implicit conversion just built (clang gives the conversion the
/// qualified type: `ImplicitCastExpr 'const long' <IntegralCast>`).
fn qualify_prvalue(s: &mut Sema<'_, '_>, e: ExprId, to: QualType) -> ExprId {
    if s.ast.types.canonical(ty(s, e)) == s.ast.types.canonical(to) {
        return e;
    }
    let n = *s.ast.exprs.get(e);
    if n.kind() == ExprKind::ImplicitCastExpr
        && !matches!(
            s.ast.exprs.cast_kind(e),
            CastKind::LValueToRValue
                | CastKind::UserDefinedConversion
                | CastKind::ConstructorConversion
        )
    {
        let ck = s.ast.exprs.cast_kind(e);
        let op = s.ast.exprs.operand(e);
        // The retyped cast keeps its base path: `S *const &r = u;` with
        // `u` a `U *` is still a derived-to-base adjustment (WP-782).
        let path = s.ast.exprs.cast_path(e);
        return raw_cast(s, ck, op, to, ValueCategory::PRValue, path);
    }
    raw_cast(s, CastKind::NoOp, e, to, ValueCategory::PRValue, None)
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
    // clang's `InitializationSequence::Perform` drops a misaligned `&m`
    // converted to a type that tolerates it (`-Waddress-of-packed-member`);
    // a simple assignment does not (`CheckSingleAssignmentConstraints`).
    if !s.misaligned.is_empty() && ctx != ConvCtx::Assignment {
        s.discard_misaligned_member_address(to, e);
    }
    if from == QualType::BOUND_MEMBER
        || (from == QualType::OVERLOAD
            && s.member_overload(e).is_some_and(|x| s.member_set_needs_call(x)))
    {
        let r = s.recover_not_called(e);
        return if s.is_err_expr(r) { Ok(r) } else { implicit_convert(s, r, to, ctx) };
    }
    if s.check_builtin_fn_use(e) || s.names_invalid_function(e) {
        let loc = s.eloc(e);
        return Ok(s.recover_err(&[e], loc));
    }
    // [over.over]/1: an object or reference of class type is not a target;
    // the overload set is the argument of the class's converting
    // constructor (`const D& d = f;` with `D(R (*)())`).
    if from == QualType::OVERLOAD && !s.overload_set_converts_to_class(e, to, IcsOptions::NONE) {
        // [over.over]: the target selects the function.
        let resolved = if ctx == ConvCtx::Condition {
            s.check_overload_use(e);
            None
        } else if ctx == ConvCtx::Assignment
            && s.overload_operand(e).is_some()
            && s.try_implicit_conversion(&s.arg_info(e), to, IcsOptions::NONE).is_bad()
        {
            // [expr.assign]/3 via [over.over]: clang's
            // `CheckSingleAssignmentConstraints` resolves silently and
            // reports the assignment.
            s.diagnose_overload_assign(e, to);
            None
        } else {
            let at = s.begin_loc(e);
            s.resolve_address_of_overloaded(
                e,
                to,
                true,
                crate::overload::address::AddrCtx::Convert,
                at,
            )
        };
        return match resolved {
            // A non-static member without an object: reported at the end
            // of the full-expression ([expr.prim.id.general]/4).
            Some(x) if s.member_refs.get(&x).is_some_and(|r| r.error) => {
                let loc = s.eloc(x);
                Ok(s.recover_err(&[x], loc))
            }
            Some(x) => implicit_convert(s, x, to, ctx),
            None => {
                let loc = s.eloc(e);
                Ok(s.recover_err(&[e], loc))
            }
        };
    }
    if s.ast.types.is_reference(to) {
        let r = bind_reference(s, e, to, ctx)?;
        exc_spec_compat(s, e, from, to, ctx)?;
        return Ok(r);
    }
    let (fr, tr) = (s.ast.types.as_record(from), s.ast.types.as_record(to));
    if tr.is_some() {
        let prvalue = s.ast.exprs.get(e).value_category() == ValueCategory::PRValue;
        if prvalue && s.ast.types.same_unqualified(from, to) {
            return Ok(e);
        }
        let at = s.begin_loc(e);
        if fr.is_some() {
            if let Some(x) = s.construct_from_class(e, to, ctx, at, crate::class::DeletedAs::Call) {
                return Ok(x);
            }
            // [over.match.copy]: an unrelated class source.
            return user_conversion(s, e, to, ctx, at);
        }
        if let Some(x) = s.convert_to_class(e, to, ctx, at, crate::class::DeletedAs::Call) {
            return Ok(x);
        }
        return Err(unsupported("initialization of a class object"));
    }
    if fr.is_some() {
        // [over.match.conv]: a conversion function.
        let at = s.begin_loc(e);
        return user_conversion(s, e, to, ctx, at);
    }
    if let Some(scs) = standard_conversion(s, e, to, ctx) {
        exc_spec_compat(s, e, from, to, ctx)?;
        return apply_standard_conversion(s, e, &scs, to);
    }
    if let Some(x) = retarget_template_id(s, e, to, ctx) {
        return implicit_convert(s, x, to, ctx);
    }
    Err(conversion_error(s, e, to, ctx))
}

/// [except.spec]/5 (C++14) for the conversions clang checks
/// (`CheckExceptionSpecCompatibility`: assignment, initialization,
/// argument passing, return; not casts): see
/// [`Sema::exc_spec_compat_error`].
fn exc_spec_compat(
    s: &Sema<'_, '_>,
    e: ExprId,
    from: QualType,
    to: QualType,
    ctx: ConvCtx,
) -> Result<(), ConvError> {
    if !matches!(
        ctx,
        ConvCtx::Initialization
            | ConvCtx::DirectInitialization
            | ConvCtx::Assignment
            | ConvCtx::Argument
            | ConvCtx::Return
    ) {
        return Ok(());
    }
    // The second standard conversion of a user-defined conversion
    // sequence is not checked (clang performs it as `AA_Converting`).
    let n = s.ast.exprs.get(e);
    if n.kind() == ExprKind::ImplicitCastExpr
        && s.ast.exprs.cast_kind(e) == CastKind::UserDefinedConversion
    {
        return Ok(());
    }
    match s.exc_spec_compat_error(from, to) {
        Some(mut err) => {
            err.at = Some(s.begin_loc(e));
            Err(err)
        }
        None => Ok(()),
    }
}

/// Converts `e` to `to` by a user-defined conversion ([class.conv];
/// `at` as for [`Sema::user_convert`]); when there is none, clang's error
/// for `ctx` with the candidates as notes.
pub(crate) fn user_conversion(
    s: &mut Sema<'_, '_>,
    e: ExprId,
    to: QualType,
    ctx: ConvCtx,
    at: Loc,
) -> Result<ExprId, ConvError> {
    if let Some(r) = s.user_convert(e, to, ctx, at) {
        return r;
    }
    let mut err = conversion_error(s, e, to, ctx);
    if matches!(
        err.id,
        DiagId::sema_no_viable_conversion | DiagId::sema_no_viable_conversion_return
    ) {
        err.notes = s.udc_failure_notes(e, to, ctx);
    }
    Err(err)
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

/// Assignment to `_Atomic(T)`, `T` a class, from a `T` or `_Atomic(T)`
/// (clang `CheckAssignmentConstraints` on the value type, C11
/// 6.5.16.1p1): the right side's lvalue conversion, then
/// `NonAtomicToAtomic`.
pub(crate) fn atomic_class_assign(s: &mut Sema<'_, '_>, to: QualType, r: ExprId) -> Option<ExprId> {
    let v = s.ast.types.as_atomic(to)?;
    s.ast.types.as_record(v)?;
    let rt = ty(s, r);
    let rv = s.ast.types.as_atomic(rt).unwrap_or(rt);
    if !s.ast.types.same_unqualified(rv, v) {
        return None;
    }
    let r = lvalue_to_rvalue(s, r);
    let to = strip_cv(s, to);
    Some(raw_cast(s, CastKind::NonAtomicToAtomic, r, to, ValueCategory::PRValue, None))
}
