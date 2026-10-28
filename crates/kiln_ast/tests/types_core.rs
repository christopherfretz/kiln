//! `TypeContext` semantics (spec/types.md): interning, reference collapsing,
//! the array-qualifier rule, canonical forms, dependent-type equivalence
//! through expression profiles, flags, sizes and the memory report.
#![allow(clippy::many_single_char_names, reason = "short-lived type handles")]

use std::mem::size_of;

use kiln_ast::ids::{DeclId, ExprId, QualType, TypeId};
use kiln_ast::mem::HeapSize;
use kiln_ast::types::{
    ConstArray, ExcSpec, FuncInfo, FuncProto, LayoutCache, NoexceptValue, Quals, RecordLayout,
    TagKeyword, TargetTypes, TemplateArg, TypeContext, TypeFlags, TypeKind, TypeNode, TypeOptions,
    TypeView,
};

const INT: QualType = QualType::INT;

#[test]
fn sizes() {
    assert_eq!(size_of::<TypeNode>(), 8);
    assert_eq!(size_of::<QualType>(), 4);
    assert_eq!(size_of::<Option<QualType>>(), 4);
    assert_eq!(size_of::<TypeId>(), 4);
    assert_eq!(size_of::<FuncProto>(), 16);
    assert_eq!(size_of::<TemplateArg>(), 12);
    assert_eq!(size_of::<ConstArray>(), 16);
}

#[test]
fn builtins_are_preinterned() {
    let ctx = TypeContext::default();
    assert_eq!(ctx.kind(TypeId::INT), TypeKind::Builtin);
    assert!(ctx.is_canonical(TypeId::INT));
    assert!(ctx.is_integral(INT));
    assert!(ctx.is_floating(QualType::DOUBLE));
    assert!(ctx.is_void(QualType::VOID));
    assert!(!ctx.exhausted());
}

#[test]
fn hash_consing() {
    let mut ctx = TypeContext::default();
    let a = ctx.pointer_to(INT);
    let n = ctx.len();
    let b = ctx.pointer_to(INT);
    assert_eq!(a, b);
    assert_eq!(ctx.len(), n, "a second lookup creates no node");
    let c = ctx.pointer_to(QualType::LONG);
    assert_ne!(a, c);
    let f1 = ctx.function(INT, &[INT, c], FuncInfo::new(), ExcSpec::None);
    let f2 = ctx.function(INT, &[INT, c], FuncInfo::new(), ExcSpec::None);
    assert_eq!(f1, f2);
    let f3 = ctx.function(INT, &[INT], FuncInfo::new(), ExcSpec::None);
    assert_ne!(f1, f3);
}

#[test]
fn sugar_canonicalizes() {
    let mut ctx = TypeContext::default();
    let td: QualType = ctx.typedef_type(DeclId::new(1), INT).into();
    assert_ne!(td, INT);
    assert_eq!(ctx.canonical(td), INT);
    assert!(ctx.same_type(td, INT));
    assert!(ctx.flags(td.ty()).contains(TypeFlags::SUGAR));
    let p1 = ctx.pointer_to(td);
    let p2 = ctx.pointer_to(INT);
    assert_ne!(p1, p2);
    assert_eq!(ctx.canonical(p1), p2);
    let ctd = ctx.qualify(td, Quals::CONST);
    assert_eq!(ctx.canonical(ctd), QualType::new(TypeId::INT, Quals::CONST.bits()));
    assert_eq!(ctx.desugar_top(td), INT);
}

#[test]
fn reference_collapsing() {
    let mut ctx = TypeContext::default();
    let l = ctx.lvalue_ref_to(INT);
    let r = ctx.rvalue_ref_to(INT);
    // T& & -> T&, T&& & -> T&, T& && -> T&, T&& && -> T&&
    assert_eq!(ctx.lvalue_ref_to(l), l);
    assert_eq!(ctx.lvalue_ref_to(r), l);
    assert_eq!(ctx.rvalue_ref_to(l), l);
    assert_eq!(ctx.rvalue_ref_to(r), r);
    // Through a typedef: typedef int& LR; LR&& is int& (sugar kept).
    let lr: QualType = ctx.typedef_type(DeclId::new(2), l).into();
    let x = ctx.rvalue_ref_to(lr);
    assert_eq!(ctx.canonical(x), l);
    let x = ctx.lvalue_ref_to(lr);
    assert_eq!(ctx.canonical(x), l);
    // cv on a reference is ignored.
    assert_eq!(ctx.qualify(l, Quals::CONST), l);
}

#[test]
fn array_qualifiers_go_to_the_element() {
    let mut ctx = TypeContext::default();
    let a = ctx.constant_array(INT, 3, None);
    let ca = ctx.qualify(a, Quals::CONST);
    let ci = ctx.qualify(INT, Quals::CONST);
    let expect = ctx.constant_array(ci, 3, None);
    assert_eq!(ca, expect, "const int[3] is an array of const int");
    assert_eq!(ctx.effective_quals(ca), Quals::CONST);
    assert_eq!(ctx.unqualified(ca), a);
    // Through sugar: typedef int Arr[3]; const Arr.
    let arr: QualType = ctx.typedef_type(DeclId::new(3), a).into();
    let carr = ctx.qualify(arr, Quals::CONST);
    assert_eq!(ctx.kind(carr.ty()), TypeKind::QualifiedArray);
    assert_eq!(ctx.canonical(carr), expect);
    assert_eq!(ctx.effective_quals(carr), Quals::CONST);
    assert_eq!(ctx.unqualified(carr), arr);
    let vcarr = ctx.qualify(carr, Quals::VOLATILE);
    let cvi = ctx.qualify(INT, Quals::CV);
    let cv3 = ctx.constant_array(cvi, 3, None);
    assert_eq!(ctx.canonical(vcarr), cv3);
    // Multi-dimensional.
    let a23 = ctx.constant_array(a, 2, None);
    let c23 = ctx.qualify(a23, Quals::CONST);
    assert_eq!(ctx.element(c23).map(|e| ctx.effective_quals(e)), Some(Quals::CONST));
    // cv on a function type is ignored.
    let f = ctx.function(QualType::VOID, &[], FuncInfo::new(), ExcSpec::None);
    assert_eq!(ctx.qualify(f, Quals::CONST), f);
}

#[test]
fn array_size_expr_is_sugar() {
    let mut ctx = TypeContext::default();
    let a = ctx.constant_array(INT, 4, Some(ExprId::new(9)));
    let b = ctx.constant_array(INT, 4, None);
    assert_ne!(a, b);
    assert_eq!(ctx.canonical(a), b);
    assert_eq!(ctx.array_size(a), Some(4));
}

#[test]
fn exception_specs_cxx17() {
    let mut ctx = TypeContext::default();
    let i = FuncInfo::new();
    let v = QualType::VOID;
    let plain = ctx.function(v, &[], i, ExcSpec::None);
    let ne = ctx.function(v, &[], i, ExcSpec::BasicNoexcept);
    let th = ctx.function(v, &[], i, ExcSpec::DynamicNone);
    let ne_true = ctx.function(
        v,
        &[],
        i,
        ExcSpec::NoexceptExpr { expr: ExprId::new(1), value: NoexceptValue::True },
    );
    let ne_false = ctx.function(
        v,
        &[],
        i,
        ExcSpec::NoexceptExpr { expr: ExprId::new(2), value: NoexceptValue::False },
    );
    assert!(!ctx.same_type(plain, ne), "noexcept is part of the type in C++17");
    assert!(ctx.same_type(ne, th), "throw() and noexcept are the same type");
    assert!(ctx.same_type(ne, ne_true));
    assert!(ctx.same_type(plain, ne_false));
    let c = ctx.canonical(ne);
    let p = ctx.proto(c).copied().expect("function");
    assert_eq!(ctx.exc_spec(&p), ExcSpec::Nothrow);
    // As written, the sugar keeps its own specification.
    let p = ctx.proto_of(th.ty()).copied().expect("function");
    assert_eq!(ctx.exc_spec(&p), ExcSpec::DynamicNone);
}

#[test]
fn exception_specs_cxx14() {
    let mut ctx =
        TypeContext::with_options(TargetTypes::default(), TypeOptions { noexcept_in_type: false });
    let i = FuncInfo::new();
    let v = QualType::VOID;
    let plain = ctx.function(v, &[], i, ExcSpec::None);
    let ne = ctx.function(v, &[], i, ExcSpec::BasicNoexcept);
    let l = ctx.type_list(&[INT]);
    let dy = ctx.function(v, &[], i, ExcSpec::Dynamic(l));
    assert!(ctx.same_type(plain, ne), "C++14: noexcept is not part of the type");
    assert!(ctx.same_type(plain, dy));
}

#[test]
fn function_canonical_drops_param_cv_and_trailing() {
    let mut ctx = TypeContext::default();
    let ci = ctx.qualify(INT, Quals::CONST);
    let a = ctx.function(INT, &[ci], FuncInfo::new(), ExcSpec::None);
    let b = ctx.function(INT, &[INT], FuncInfo::new(), ExcSpec::None);
    assert!(ctx.same_type(a, b));
    let t = ctx.function(INT, &[INT], FuncInfo::new().with_trailing_return(true), ExcSpec::None);
    assert!(ctx.same_type(t, b));
    let c = ctx.function(INT, &[INT], FuncInfo::new().with_variadic(true), ExcSpec::None);
    assert!(!ctx.same_type(c, b));
}

#[test]
fn template_parameters_and_dependence() {
    let mut ctx = TypeContext::default();
    let t: QualType = ctx.template_type_parm(DeclId::new(10), 0, 0, false).into();
    let u: QualType = ctx.template_type_parm(DeclId::new(11), 0, 0, false).into();
    assert_ne!(t, u);
    assert!(ctx.same_type(t, u), "same depth and index: same canonical parameter");
    assert!(matches!(
        ctx.canonical_view(t),
        TypeView::CanonTemplateTypeParm { depth: 0, index: 0, pack: false }
    ));
    let f = ctx.flags(t.ty());
    assert!(f.contains(TypeFlags::DEPENDENT));
    assert!(f.contains(TypeFlags::INSTANTIATION_DEPENDENT));
    assert!(!f.contains(TypeFlags::UNEXPANDED_PACK));
    let p = ctx.pointer_to(t);
    assert!(ctx.is_dependent(p));
    // Packs.
    let ts: QualType = ctx.template_type_parm(DeclId::new(12), 0, 1, true).into();
    assert!(ctx.flags(ts.ty()).contains(TypeFlags::UNEXPANDED_PACK));
    let pts = ctx.pointer_to(ts);
    assert!(ctx.flags(pts.ty()).contains(TypeFlags::UNEXPANDED_PACK));
    let e = ctx.pack_expansion(pts, None);
    let fe = ctx.flags(e.ty());
    assert!(!fe.contains(TypeFlags::UNEXPANDED_PACK));
    assert!(fe.contains(TypeFlags::DEPENDENT));
    // A non-dependent type has no flags.
    let q = ctx.pointer_to(INT);
    assert_eq!(ctx.flags(q.ty()).without(TypeFlags::SUGAR), TypeFlags::NONE);
}

#[test]
fn dependent_equivalence_through_profiles() {
    let mut ctx = TypeContext::default();
    let t: QualType = ctx.template_type_parm(DeclId::new(10), 0, 0, false).into();
    let dep = TypeFlags::DEPENDENT_INSTANTIATION;
    // `T[sizeof(T)]` written twice (two expressions, one profile).
    let e1 = ExprId::new(1);
    let e2 = ExprId::new(2);
    let p1 = ctx.profile(&[7, 1, 0, 0], e1, dep);
    let p2 = ctx.profile(&[7, 1, 0, 0], e2, dep);
    assert_eq!(p1, p2);
    assert_eq!(ctx.profile_expr(p1), Some(e1), "the first expression is kept");
    let a1 = ctx.dependent_sized_array(t, e1, p1);
    let a2 = ctx.dependent_sized_array(t, e2, p2);
    assert_ne!(a1, a2, "each written array keeps its expression");
    assert!(ctx.same_type(a1, a2));
    // A different profile is a different type.
    let p3 = ctx.profile(&[7, 2, 0, 0], ExprId::new(3), dep);
    let a3 = ctx.dependent_sized_array(t, ExprId::new(3), p3);
    assert!(!ctx.same_type(a1, a3));
    // decltype(expr) likewise.
    let d1 = ctx.decltype_type(e1, QualType::ERROR, Some(p1));
    let d2 = ctx.decltype_type(e2, QualType::ERROR, Some(p1));
    assert!(ctx.same_type(d1, d2));
    assert!(ctx.is_dependent(d1));
    // noexcept(expr) dependent on a template parameter.
    let i = FuncInfo::new();
    let f1 = ctx.function(
        QualType::VOID,
        &[],
        i,
        ExcSpec::NoexceptExpr { expr: e1, value: NoexceptValue::Dependent(p1) },
    );
    let f2 = ctx.function(
        QualType::VOID,
        &[],
        i,
        ExcSpec::NoexceptExpr { expr: e2, value: NoexceptValue::Dependent(p2) },
    );
    assert!(ctx.same_type(f1, f2));
    assert!(ctx.is_dependent(f1));
}

#[test]
fn template_specializations() {
    let mut ctx = TypeContext::default();
    let w = ctx.template_name_of(DeclId::new(20));
    let t: QualType = ctx.template_type_parm(DeclId::new(10), 0, 0, false).into();
    let u: QualType = ctx.template_type_parm(DeclId::new(11), 0, 0, false).into();
    let at = ctx.arg_list(&[TemplateArg::ty(t)]);
    let au = ctx.arg_list(&[TemplateArg::ty(u)]);
    let wt = ctx.template_specialization(w, at, None, false);
    let wu = ctx.template_specialization(w, au, None, false);
    assert_ne!(wt, wu);
    assert!(ctx.same_type(wt, wu), "W<T> and W<U> at the same position");
    assert!(ctx.is_dependent(wt));
    // A non-dependent specialization canonicalizes to its record.
    let rec: QualType = ctx.record_type(DeclId::new(21)).into();
    let ai = ctx.arg_list(&[TemplateArg::ty(INT)]);
    let wi = ctx.template_specialization(w, ai, Some(rec), false);
    assert_eq!(ctx.canonical(wi), rec);
    assert!(!ctx.is_dependent(wi));
    // The injected-class-name is sugar for the dependent specialization.
    let icn: QualType = ctx.injected_class_name(DeclId::new(20), wt.ty()).into();
    assert!(ctx.same_type(icn, wt));
    // Elaborated with the default keyword canonicalizes away.
    let e = ctx.elaborated(TagKeyword::Struct, None, rec);
    assert_eq!(ctx.canonical(e), rec);
}

#[test]
fn substitution_sugar() {
    let mut ctx = TypeContext::default();
    let s = ctx.subst_template_type_parm(INT, DeclId::new(5), 0, None);
    assert_ne!(s, INT);
    assert_eq!(ctx.canonical(s), INT);
    assert!(!ctx.is_dependent(s));
}

#[test]
fn record_and_enum_types_are_distinct_and_stable() {
    let mut ctx = TypeContext::default();
    let r1 = ctx.record_type(DeclId::new(1));
    let r2 = ctx.record_type(DeclId::new(1));
    let e1 = ctx.enum_type(DeclId::new(1));
    assert_eq!(r1, r2);
    assert_ne!(r1, e1);
    assert_eq!(ctx.as_record(r1.into()), Some(DeclId::new(1)));
    assert_eq!(ctx.as_enum(e1.into()), Some(DeclId::new(1)));
    assert!(ctx.is_canonical(r1));
}

#[test]
fn memory_report() {
    let mut ctx = TypeContext::default();
    let before = ctx.heap_bytes().live_bytes();
    let mut q = INT;
    for i in 0..1000 {
        q = ctx.pointer_to(q);
        let _ = ctx.constant_array(q, i, None);
    }
    let r = ctx.heap_bytes();
    assert!(r.live_bytes() > before);
    assert!(r.reserved_bytes() >= r.live_bytes());
    let nodes = r.row("nodes").expect("nodes row");
    assert!(nodes.count >= 2000);
    assert_eq!(nodes.live_bytes, nodes.count * 8);
    assert!(r.row("intern").is_some());
    let text = r.to_string();
    assert!(text.contains("nodes"));
}

#[test]
fn layout_cache() {
    let mut cache = LayoutCache::default();
    assert!(cache.is_empty());
    let l = RecordLayout {
        size: 8,
        align: 4,
        preferred_align: 4,
        data_size: 8,
        nv_size: 8,
        nv_align: 4,
        fields: vec![0, 32].into_boxed_slice(),
        bases: Box::new([]),
        vbases: Box::new([]),
        vptr: None,
        primary_base: None,
    };
    cache.insert(DeclId::new(4), l);
    assert_eq!(cache.get(DeclId::new(4)).map(|l| l.size), Some(8));
    assert_eq!(cache.len(), 1);
    assert!(cache.heap_bytes().live_bytes() > 0);
}

#[test]
fn exhaustion_is_not_a_panic() {
    // Extremely large pack-expansion counts overflow the node encoding:
    // the context reports exhaustion and returns the error type.
    let mut ctx = TypeContext::default();
    let t: QualType = ctx.template_type_parm(DeclId::new(10), 0, 0, true).into();
    let e = ctx.pack_expansion(t, Some(u32::MAX));
    assert_eq!(e, QualType::ERROR);
    assert!(ctx.exhausted());
}
