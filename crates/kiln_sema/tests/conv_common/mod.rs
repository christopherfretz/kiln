//! Shared helpers for the `conv_*` tests: a `Sema` over an x86-64 (or
//! aarch64) target, AST construction shorthands, and diagnostic capture.
#![allow(dead_code, reason = "each test binary uses a subset")]
#![allow(
    clippy::many_single_char_names,
    clippy::items_after_statements,
    reason = "s/n/t: sema, name, type; local helpers sit next to their use"
)]

use std::sync::Arc;

use kiln_ast::decl::{
    Access, BaseSpec, DeclKind, DefinitionBits, EnumData, FnSpec, TagKind, VarSpec,
};
use kiln_ast::expr::{CastKind, ExprFlags, ExprKind, NonOdrUse, Radix, UnaryOp, ValueCategory};
use kiln_ast::names::DeclName;
use kiln_ast::types::{BuiltinType, ExcSpec, FloatFormat, FuncInfo, TargetTypes, TypeContext};
use kiln_ast::{Ast, DeclHead, DeclId, ExprId, QualType, TypeId};
use kiln_diag::{Diag, DiagEngine, DiagOptions, MemorySink};
use kiln_sema::Sema;
use kiln_sema::conv::ConvError;
use kiln_source::symbol::Interner;
use kiln_source::{Loc, SourceManager};

/// No location.
pub const L: Loc = Loc::INVALID;

/// `x86_64-linux-gnu`: the target every expectation was taken on with
/// `clang++-19 --target=x86_64-linux-gnu -std=c++26`.
pub fn x86_64() -> TargetTypes {
    let mut t = TargetTypes::default();
    let set = |t: &mut TargetTypes, b: BuiltinType, sz: u16, al: u16| {
        t.size_align[b as usize] = (sz, al);
    };
    use BuiltinType as B;
    for b in [B::Bool, B::Char, B::SChar, B::UChar, B::Char8] {
        set(&mut t, b, 1, 1);
    }
    for b in [B::Short, B::UShort, B::Char16, B::Float16, B::BFloat16, B::Half] {
        set(&mut t, b, 2, 2);
    }
    for b in [B::Int, B::UInt, B::WChar, B::Char32, B::Float] {
        set(&mut t, b, 4, 4);
    }
    for b in [B::Long, B::ULong, B::LongLong, B::ULongLong, B::Double, B::NullPtr] {
        set(&mut t, b, 8, 8);
    }
    for b in [B::Int128, B::UInt128, B::LongDouble, B::Float128] {
        set(&mut t, b, 16, 16);
    }
    t.char_signed = true;
    t.wchar_signed = true;
    t.long_double = FloatFormat::X87Extended;
    t.size_t = TypeId::ULONG;
    t.ptrdiff_t = TypeId::LONG;
    t.intptr_t = TypeId::LONG;
    t.pointer_bytes = 8;
    t
}

/// `aarch64-linux-gnu`: unsigned `char` and `wchar_t`, binary128
/// `long double`.
pub fn aarch64() -> TargetTypes {
    let mut t = x86_64();
    t.char_signed = false;
    t.wchar_signed = false;
    t.long_double = FloatFormat::IeeeQuad;
    t
}

/// Runs `f` with a fresh `Sema` for `target` and the sink its
/// diagnostics go to.
pub fn with_target<R>(
    target: TargetTypes,
    f: impl FnOnce(&mut Sema<'_, '_>, &MemorySink) -> R,
) -> R {
    let sm = SourceManager::new();
    let sink = MemorySink::new();
    let opts = DiagOptions::default();
    let engine = DiagEngine::new(&sm, &opts, sink.clone());
    let ast = Ast::new(Arc::new(Interner::new()), TypeContext::new(target.clone()));
    let mut s = Sema::new(ast, &engine, target);
    f(&mut s, &sink)
}

/// [`with_target`] on x86-64.
pub fn with_sema<R>(f: impl FnOnce(&mut Sema<'_, '_>, &MemorySink) -> R) -> R {
    with_target(x86_64(), f)
}

/// Emits `d` and returns what was printed: `level: message` per
/// diagnostic (notes included).
pub fn emitted(s: &Sema<'_, '_>, sink: &MemorySink, d: Diag) -> Vec<String> {
    let _ = sink.take();
    s.emit(d);
    fn walk(d: &kiln_diag::Diagnostic, out: &mut Vec<String>) {
        out.push(format!("{}: {}", d.level.text_name(), d.message));
        for n in &d.notes {
            walk(n, out);
        }
    }
    let mut out = Vec::new();
    for d in sink.take() {
        walk(&d, &mut out);
    }
    out
}

/// The message `err` prints when emitted at `e`.
pub fn message(s: &Sema<'_, '_>, sink: &MemorySink, err: &ConvError, e: ExprId) -> String {
    emitted(s, sink, err.diag(s, e)).join("\n")
}

/// A name.
pub fn name(s: &Sema<'_, '_>, n: &str) -> DeclName {
    DeclName::ident(s.ast.symbols().intern(n))
}

fn head(s: &Sema<'_, '_>, n: &str) -> DeclHead {
    DeclHead::new(s.ast.tu, name(s, n), L)
}

/// A variable `n` of type `ty` at namespace scope.
pub fn var(s: &mut Sema<'_, '_>, n: &str, ty: QualType) -> DeclId {
    let h = head(s, n);
    s.ast.builder().var(h, ty, VarSpec::new(), None)
}

/// An lvalue naming a fresh variable of type `ty` (dump: `DeclRefExpr
/// 'ty' lvalue Var 'n'`).
pub fn lvalue(s: &mut Sema<'_, '_>, n: &str, ty: QualType) -> ExprId {
    let v = var(s, n, ty);
    s.ast.builder().decl_ref(v, ty, L, None, NonOdrUse::None, ExprFlags::NONE)
}

/// An xvalue of type `ty` (`static_cast<T&&>(lv)`).
pub fn xvalue(s: &mut Sema<'_, '_>, n: &str, ty: QualType) -> ExprId {
    let lv = lvalue(s, n, ty);
    s.ast.builder().cast(
        ExprKind::CXXStaticCastExpr,
        CastKind::NoOp,
        lv,
        ty,
        ValueCategory::XValue,
        None,
        L,
        ExprFlags::NONE,
    )
}

/// A prvalue of type `ty` that is not a constant (a call of a function
/// returning `ty`).
pub fn prvalue(s: &mut Sema<'_, '_>, n: &str, ty: QualType) -> ExprId {
    let ft = fn_type(s, ty, &[]);
    let h = head(s, n);
    let f = s.ast.builder().function(h, DeclKind::Function, ft, FnSpec::new(), None);
    let fp = s.ast.types.pointer_to(ft);
    let mut b = s.ast.builder();
    let r = b.decl_ref(f, ft, L, None, NonOdrUse::None, ExprFlags::NONE);
    let callee = b.implicit_cast(CastKind::FunctionToPointerDecay, r, fp, ValueCategory::PRValue);
    b.call(callee, &[], ty, ValueCategory::PRValue, L, L, ExprFlags::NONE)
}

/// An lvalue naming function `n` of type `ft`.
pub fn function_ref(s: &mut Sema<'_, '_>, n: &str, ft: QualType) -> ExprId {
    let h = head(s, n);
    let f = s.ast.builder().function(h, DeclKind::Function, ft, FnSpec::new(), None);
    s.ast.builder().decl_ref(f, ft, L, None, NonOdrUse::None, ExprFlags::NONE)
}

/// An integer literal of type `ty`.
pub fn int_lit(s: &mut Sema<'_, '_>, ty: QualType, v: u64) -> ExprId {
    s.ast.builder().int_literal(ty, v, Radix::Decimal, L)
}

/// `-lit` for an `int` literal.
pub fn neg_int(s: &mut Sema<'_, '_>, v: u64) -> ExprId {
    let l = int_lit(s, QualType::INT, v);
    s.ast.builder().unary(UnaryOp::Minus, l, QualType::INT, L, ExprFlags::NONE)
}

/// A floating literal of type `ty` with value `v` (exactly
/// representable in `ty`'s format for the tests that use it).
pub fn float_lit(s: &mut Sema<'_, '_>, ty: QualType, v: f64) -> ExprId {
    use kiln_ast::consts::{FloatKind, Scalar};
    let (kind, bits) = match ty {
        QualType::FLOAT => (FloatKind::Single, u128::from((v as f32).to_bits())),
        _ => (FloatKind::Double, u128::from(v.to_bits())),
    };
    let c = s.ast.consts.push_scalar(Scalar::Float { ty, kind, bits }).expect("intern");
    s.ast.builder().float_literal(ty, c, L)
}

/// `true` / `false`.
pub fn bool_lit(s: &mut Sema<'_, '_>, v: bool) -> ExprId {
    s.ast.builder().bool_literal(QualType::BOOL, v, L)
}

/// `nullptr`.
pub fn nullptr(s: &mut Sema<'_, '_>) -> ExprId {
    s.ast.builder().nullptr_literal(QualType::NULLPTR, L)
}

/// A function type `ret(params...)`.
pub fn fn_type(s: &mut Sema<'_, '_>, ret: QualType, params: &[QualType]) -> QualType {
    s.ast.types.function(ret, params, FuncInfo::new(), ExcSpec::None)
}

/// A `noexcept` function type `ret(params...) noexcept`.
pub fn noexcept_fn_type(s: &mut Sema<'_, '_>, ret: QualType, params: &[QualType]) -> QualType {
    s.ast.types.function(ret, params, FuncInfo::new(), ExcSpec::BasicNoexcept)
}

/// `T*`.
pub fn ptr(s: &mut Sema<'_, '_>, t: QualType) -> QualType {
    s.ast.types.pointer_to(t)
}

/// `const T`.
pub fn cnst(s: &mut Sema<'_, '_>, t: QualType) -> QualType {
    s.ast.types.qualify(t, kiln_ast::types::Quals::CONST)
}

/// `volatile T`.
pub fn vol(s: &mut Sema<'_, '_>, t: QualType) -> QualType {
    s.ast.types.qualify(t, kiln_ast::types::Quals::VOLATILE)
}

/// `T&`.
pub fn lref(s: &mut Sema<'_, '_>, t: QualType) -> QualType {
    s.ast.types.lvalue_ref_to(t)
}

/// `T&&`.
pub fn rref(s: &mut Sema<'_, '_>, t: QualType) -> QualType {
    s.ast.types.rvalue_ref_to(t)
}

/// `T[n]`.
pub fn array(s: &mut Sema<'_, '_>, t: QualType, n: u64) -> QualType {
    s.ast.types.constant_array(t, n, None)
}

/// `T C::*` for class type `c`.
pub fn mem_ptr(s: &mut Sema<'_, '_>, t: QualType, c: QualType) -> QualType {
    s.ast.types.member_pointer(t, c.ty())
}

/// A typedef `n` for `t` (sugar).
pub fn typedef(s: &mut Sema<'_, '_>, n: &str, t: QualType) -> QualType {
    let h = head(s, n);
    let d = s.ast.builder().typedef(h, DeclKind::Typedef, t, None);
    s.ast.decls.get(d).ty().expect("typedef type")
}

/// A complete `struct n` with the given `(base type, virtual)` bases and
/// no members; returns its type.
pub fn class(s: &mut Sema<'_, '_>, n: &str, bases: &[(QualType, bool)]) -> QualType {
    let h = head(s, n);
    let r = s.ast.builder().record(h, TagKind::Struct, None);
    s.ast.builder().begin_record_definition(r).expect("first definition");
    let specs: Vec<BaseSpec> = bases
        .iter()
        .map(|&(ty, is_virtual)| BaseSpec {
            ty,
            loc: L,
            is_virtual,
            access: Access::Public,
            pack_expansion: false,
        })
        .collect();
    let ast = &mut s.ast;
    ast.decls.set_bases(r, &specs, &mut ast.lists).expect("bases once");
    s.ast.builder().complete_record_definition(r, DefinitionBits::empty());
    s.ast.decls.get(r).ty().expect("record type")
}

/// An enumeration `n`: `fixed` is `(underlying, promotion)` for a fixed
/// underlying type, else `computed` is used at completion.
pub fn enumeration(
    s: &mut Sema<'_, '_>,
    n: &str,
    scoped: bool,
    fixed: Option<QualType>,
    computed: (QualType, QualType),
) -> QualType {
    let h = head(s, n);
    let flags = if scoped { EnumData::SCOPED } else { 0 };
    let fixed_pair = fixed.map(|u| {
        let p = kiln_sema::conv::promoted_type(s, u, None).unwrap_or(u);
        (u, p)
    });
    let e = s.ast.builder().enumeration(h, flags, fixed_pair, None);
    s.ast.builder().begin_enum_definition(e).expect("first definition");
    let comp = if fixed.is_some() { None } else { Some(computed) };
    s.ast.builder().complete_enum_definition(e, comp).expect("types once");
    s.ast.decls.get(e).ty().expect("enum type")
}

/// An lvalue `obj.n` naming a bit-field of `width` bits and type `ty`
/// in a fresh struct.
pub fn bit_field(s: &mut Sema<'_, '_>, n: &str, ty: QualType, width: u64) -> ExprId {
    let rh = head(s, &format!("{n}_holder"));
    let r = s.ast.builder().record(rh, TagKind::Struct, None);
    s.ast.builder().begin_record_definition(r).expect("first definition");
    let w = int_lit(s, QualType::INT, width);
    let w = s.ast.builder().constant_expr(w, None);
    let fh = DeclHead::new(r, name(s, n), L);
    let f = s.ast.builder().field(fh, ty, Some(w), 0);
    s.ast.builder().complete_record_definition(r, DefinitionBits::empty());
    let rt = s.ast.decls.get(r).ty().expect("record type");
    let obj = lvalue(s, &format!("{n}_obj"), rt);
    s.ast.builder().member(obj, f, ty, false, L, None)
}

/// The node shape `e` has: `(kind, cast kind if a cast, type, category)`.
pub fn shape(s: &Sema<'_, '_>, e: ExprId) -> (ExprKind, Option<CastKind>, QualType, ValueCategory) {
    let n = s.ast.exprs.get(e);
    let ck = matches!(n.kind(), ExprKind::ImplicitCastExpr).then(|| s.ast.exprs.cast_kind(e));
    (n.kind(), ck, n.ty(), n.value_category())
}

/// The chain of implicit casts on top of `e`, outermost first, as
/// `(cast kind, type spelled as clang prints it, category)`, stopping at
/// the first non-implicit-cast node.
pub fn casts(s: &Sema<'_, '_>, e: ExprId) -> Vec<(CastKind, String, ValueCategory)> {
    let mut out = Vec::new();
    let mut cur = e;
    while s.ast.exprs.get(cur).kind() == ExprKind::ImplicitCastExpr {
        let n = s.ast.exprs.get(cur);
        out.push((s.ast.exprs.cast_kind(cur), s.ty_str(n.ty()), n.value_category()));
        cur = s.ast.exprs.operand(cur);
    }
    out
}

/// The class a pointer, member pointer or class type designates.
fn class_of(s: &Sema<'_, '_>, q: QualType) -> Option<DeclId> {
    use kiln_ast::types::TypeView;
    let c = s.ast.types.canonical(q);
    match s.ast.types.view(c.ty()) {
        TypeView::Pointer(p) => s.ast.types.as_record(p),
        TypeView::MemberPointer { class, .. } => s.ast.types.as_record(QualType::unqual(class)),
        _ => s.ast.types.as_record(c),
    }
}

/// clang's base-path suffix ` (C -> virtual B)` for the cast `e`, walked
/// from `derived`.
fn path_suffix(s: &Sema<'_, '_>, e: ExprId, derived: QualType) -> String {
    let Some(list) = s.ast.exprs.cast_path(e) else { return String::new() };
    let mut cur = class_of(s, derived);
    let mut names = Vec::new();
    for &i in s.ast.lists.get(list) {
        let Some(c) = cur else { break };
        let def = s.ast.definition(c).unwrap_or(c);
        let idx =
            s.ast.decls.record(def).map(|r| s.ast.lists.get(r.bases).to_vec()).unwrap_or_default();
        let spec = idx
            .iter()
            .zip(s.ast.decls.bases(def, &s.ast.lists))
            .find(|(j, _)| **j == i)
            .map(|(_, b)| *b)
            .expect("path step names a base of the current class");
        let v = if spec.is_virtual { "virtual " } else { "" };
        names.push(format!("{v}{}", s.ty_str(spec.ty)));
        cur = s.ast.types.as_record(spec.ty);
    }
    if names.is_empty() { String::new() } else { format!(" ({})", names.join(" -> ")) }
}

/// `casts` rendered as clang's dump fragments: `'T' <Kind>` per cast,
/// with ` lvalue`/` xvalue` for glvalues and the base path of
/// derived/base casts, outermost first.
pub fn dump_casts(s: &Sema<'_, '_>, e: ExprId) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = e;
    while s.ast.exprs.get(cur).kind() == ExprKind::ImplicitCastExpr {
        let n = s.ast.exprs.get(cur);
        let ck = s.ast.exprs.cast_kind(cur);
        let sub = s.ast.exprs.operand(cur);
        let cat = match n.value_category() {
            ValueCategory::PRValue => "",
            ValueCategory::LValue => " lvalue",
            ValueCategory::XValue => " xvalue",
        };
        let derived = match ck {
            CastKind::DerivedToBase | CastKind::UncheckedDerivedToBase => s.ast.exprs.get(sub).ty(),
            _ => n.ty(),
        };
        let path = path_suffix(s, cur, derived);
        out.push(format!("'{}'{cat} <{ck:?}{path}>", s.ty_str(n.ty())));
        cur = sub;
    }
    out
}

/// The innermost expression under `e`'s implicit casts.
pub fn under_casts(s: &Sema<'_, '_>, e: ExprId) -> ExprId {
    let mut cur = e;
    while s.ast.exprs.get(cur).kind() == ExprKind::ImplicitCastExpr {
        cur = s.ast.exprs.operand(cur);
    }
    cur
}

/// The type id of a builtin.
pub fn builtin(b: BuiltinType) -> QualType {
    b.qual_type()
}
