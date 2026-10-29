//! Unit tests for the structural deduction core (`Sema::deduce`,
//! `Sema::deduce_args`, spec sema-templates §6.2) on directly built
//! canonical types, and the clang-worded failure notes (§10.4).
#![allow(clippy::many_single_char_names, reason = "s/p/a/d: sema, P, A, deduced")]

mod conv_common;

use conv_common::{
    L, array, class, cnst, emitted, fn_type, lref, mem_ptr, name, ptr, rref, vol, with_sema,
};
use kiln_ast::consts::{IntBits, Scalar};
use kiln_ast::decl::{DeclKind, TagKind};
use kiln_ast::types::TemplateArg;
use kiln_ast::{DeclHead, DeclId, QualType};
use kiln_sema::Sema;
use kiln_sema::templ::deduce::{DeducedArgs, TdfFlags, Tdk};

/// The canonical `type-parameter-<depth>-<index>`.
fn parm(s: &mut Sema<'_, '_>, depth: u32, index: u32) -> QualType {
    QualType::unqual(s.ast_mut().types.canon_template_type_parm(depth, index, false))
}

fn deduce(s: &mut Sema<'_, '_>, p: QualType, a: QualType, n: usize) -> (Tdk, DeducedArgs) {
    let mut d = DeducedArgs::new(0, n);
    let r = s.deduce(p, a, TdfFlags::NONE, &mut d);
    (r, d)
}

/// The note `tdk` gives for `tmpl`, emitted under a carrier error and
/// returned without it.
fn note_text(
    s: &mut Sema<'_, '_>,
    sink: &kiln_diag::MemorySink,
    tmpl: DeclId,
    tdk: Tdk,
    d: &DeducedArgs,
) -> Vec<String> {
    let note = s.deduction_failure_note(tmpl, tdk, d, L).expect("a note");
    let carrier =
        s.diag(kiln_diag::DiagId::sema_partial_spec_ambiguous, L).arg("S").with_note(note);
    let mut out = emitted(s, sink, carrier);
    out.remove(0);
    out
}

fn ty_arg(s: &Sema<'_, '_>, q: QualType) -> TemplateArg {
    TemplateArg::ty(s.ast.types.canonical(q))
}

/// `template<class T, class U> struct S;` (depth 0).
fn two_param_template(s: &mut Sema<'_, '_>) -> DeclId {
    let tu = s.ast.tu;
    let (nt, nu, ns) = (name(s, "T"), name(s, "U"), name(s, "S"));
    let mut b = s.ast.builder();
    let t = b.template_type_parm(DeclHead::new(tu, nt, L), 0, 0, false, false);
    let u = b.template_type_parm(DeclHead::new(tu, nu, L), 0, 1, false, false);
    let pl = b.template_param_list(L, L, L, 0, &[t, u], None);
    let r = b.record(DeclHead::new(tu, ns, L), TagKind::Struct, None);
    b.template_decl(DeclKind::ClassTemplate, r, pl, None)
}

/// [temp.deduct.type]/8: `T*` from `int*` deduces `T = int`.
#[test]
fn pointer_deduces_pointee() {
    with_sema(|s, _| {
        let t = parm(s, 0, 0);
        let (p, a) = (ptr(s, t), ptr(s, QualType::INT));
        let (r, d) = deduce(s, p, a, 1);
        assert_eq!(r, Tdk::Success);
        assert_eq!(d.args[0], ty_arg(s, QualType::INT));
        assert_eq!(d.first_undeduced(), None);
    });
}

/// [temp.deduct.type]/8 (`cv T`): `const T` from `const volatile int`
/// deduces `T = volatile int`.
#[test]
fn cv_param_strips_its_qualifiers() {
    with_sema(|s, _| {
        let t = parm(s, 0, 0);
        let p = cnst(s, t);
        let vi = vol(s, QualType::INT);
        let a = cnst(s, vi);
        let (r, d) = deduce(s, p, a, 1);
        assert_eq!(r, Tdk::Success);
        assert_eq!(d.args[0], ty_arg(s, vi));
    });
}

/// [temp.deduct.type]/8 (`cv T`): `const T` against `int` fails as
/// `Underqualified` (`P` is canonical here, so the note prints the
/// canonical parameter).
#[test]
fn cv_param_against_unqualified_is_underqualified() {
    with_sema(|s, sink| {
        let t = parm(s, 0, 0);
        let p = cnst(s, t);
        let (r, d) = deduce(s, p, QualType::INT, 2);
        assert_eq!(r, Tdk::Underqualified);
        assert_eq!(d.info.param, Some(0));
        let tmpl = two_param_template(s);
        assert_eq!(
            note_text(s, sink, tmpl, r, &d),
            [
                "note: candidate template ignored: cannot deduce a type for 'T' that would make 'const type-parameter-0-0' equal 'int'"
            ]
        );
    });
}

/// [temp.deduct.type]/2: `T(*)(T)` from `int(*)(double)` deduces `T`
/// two ways: `Inconsistent`, noted with both types.
#[test]
fn function_pointer_conflicting_deductions() {
    with_sema(|s, sink| {
        let t = parm(s, 0, 0);
        let pf = fn_type(s, t, &[t]);
        let af = fn_type(s, QualType::INT, &[QualType::DOUBLE]);
        let (p, a) = (ptr(s, pf), ptr(s, af));
        let (r, d) = deduce(s, p, a, 2);
        assert_eq!(r, Tdk::Inconsistent);
        assert_eq!(d.info.param, Some(0));
        let tmpl = two_param_template(s);
        let got = note_text(s, sink, tmpl, r, &d);
        assert_eq!(got.len(), 1);
        assert!(
            got[0].starts_with(
                "note: candidate template ignored: deduced conflicting types for parameter 'T' ("
            ),
            "{got:?}"
        );
    });
}

/// [temp.deduct.type]/8 (`T(U)`): the return and parameter types are
/// deduced independently; a parameter's top-level `const` is not part of
/// the function type ([dcl.fct]/5).
#[test]
fn function_type_deduces_return_and_params() {
    with_sema(|s, _| {
        let (t, u) = (parm(s, 0, 0), parm(s, 0, 1));
        let p = fn_type(s, t, &[u]);
        let cc = cnst(s, QualType::CHAR);
        let a = fn_type(s, QualType::LONG, &[cc]);
        let (r, d) = deduce(s, p, a, 2);
        assert_eq!(r, Tdk::Success);
        assert_eq!(d.args, [ty_arg(s, QualType::LONG), ty_arg(s, QualType::CHAR)]);
    });
}

/// [temp.deduct.type]/8 (`T C::*`): both the member type and the class
/// are deduced.
#[test]
fn member_pointer_deduces_class_and_member() {
    with_sema(|s, _| {
        let (t, u) = (parm(s, 0, 0), parm(s, 0, 1));
        let c = class(s, "C", &[]);
        let p = mem_ptr(s, t, u);
        let a = mem_ptr(s, QualType::INT, c);
        let (r, d) = deduce(s, p, a, 2);
        assert_eq!(r, Tdk::Success);
        assert_eq!(d.args, [ty_arg(s, QualType::INT), ty_arg(s, c)]);
    });
}

/// [temp.deduct.type]/1: `T&` does not match `int&&`; `T&&` matches it.
#[test]
fn reference_kinds_must_match() {
    with_sema(|s, _| {
        let t = parm(s, 0, 0);
        let (pl, pr) = (lref(s, t), rref(s, t));
        let ar = rref(s, QualType::INT);
        let (r, _) = deduce(s, pl, ar, 1);
        assert_ne!(r, Tdk::Success);
        let (r, d) = deduce(s, pr, ar, 1);
        assert_eq!(r, Tdk::Success);
        assert_eq!(d.args[0], ty_arg(s, QualType::INT));
    });
}

/// [temp.deduct.type]/8 (`T[i]`): the element type of a bounded array.
#[test]
fn array_element_type() {
    with_sema(|s, _| {
        let t = parm(s, 0, 0);
        let p = array(s, t, 4);
        let a4 = array(s, QualType::SHORT, 4);
        let (r, d) = deduce(s, p, a4, 1);
        assert_eq!(r, Tdk::Success);
        assert_eq!(d.args[0], ty_arg(s, QualType::SHORT));
        let a5 = array(s, QualType::SHORT, 5);
        let (r, _) = deduce(s, p, a5, 1);
        assert_ne!(r, Tdk::Success);
    });
}

/// [temp.deduct.type]/1: a non-dependent `P` must equal `A`; the
/// mismatch is noted with both sides.
#[test]
fn non_dependent_mismatch_is_noted() {
    with_sema(|s, sink| {
        let t = parm(s, 0, 0);
        let pf = fn_type(s, t, &[QualType::INT]);
        let af = fn_type(s, QualType::INT, &[QualType::LONG]);
        let (r, d) = deduce(s, pf, af, 2);
        assert_eq!(r, Tdk::NonDeducedMismatch);
        let tmpl = two_param_template(s);
        assert_eq!(
            note_text(s, sink, tmpl, r, &d),
            ["note: candidate template ignored: could not match 'int' against 'long'"]
        );
    });
}

/// [temp.deduct.type]/2 (only the template's own parameters are
/// deduced): a parameter of an enclosing depth is left alone.
#[test]
fn other_depth_parameters_are_not_deduced() {
    with_sema(|s, _| {
        let outer = parm(s, 1, 0);
        let p = ptr(s, outer);
        let a = ptr(s, QualType::INT);
        let (r, d) = deduce(s, p, a, 1);
        assert_eq!(r, Tdk::Success);
        assert_eq!(d.first_undeduced(), Some(0));
    });
}

/// [temp.deduct.type]/9: argument lists deduce element-wise; a value `P`
/// matches only an equal value `A`, and a repeated `T` must agree.
#[test]
fn deduce_args_lists() {
    with_sema(|s, sink| {
        let t = parm(s, 0, 0);
        let v = |s: &mut Sema<'_, '_>, n: i128| {
            let bits = IntBits::from_i128(32, n);
            let c = s
                .ast_mut()
                .consts
                .push_scalar(Scalar::Int { ty: QualType::INT, signed: true, bits })
                .expect("interned");
            TemplateArg::value(QualType::INT, c)
        };
        let (v3, v3b, v4) = (v(s, 3), v(s, 3), v(s, 4));
        let ta = TemplateArg::ty(t);
        let ia = TemplateArg::ty(QualType::INT);
        let la = TemplateArg::ty(QualType::LONG);

        let mut d = DeducedArgs::new(0, 1);
        assert_eq!(s.deduce_args(&[ta, v3], &[ia, v3b], true, &mut d, L), Tdk::Success);
        assert_eq!(d.args[0], ia);

        let mut d = DeducedArgs::new(0, 1);
        assert_ne!(s.deduce_args(&[ta, v3], &[ia, v4], true, &mut d, L), Tdk::Success);

        let mut d = DeducedArgs::new(0, 1);
        assert_eq!(s.deduce_args(&[ta, ta], &[ia, la], true, &mut d, L), Tdk::Inconsistent);
        assert!(sink.take().is_empty(), "deduction itself emits nothing");
    });
}

/// [temp.deduct.type]/9 (`TT<T>`): a template-id deduces through a
/// complete specialization's arguments; tested via the dependent form.
#[test]
fn incomplete_note_names_first_undeduced() {
    with_sema(|s, sink| {
        let t = parm(s, 0, 0);
        let (r, d) = deduce(s, t, QualType::INT, 2);
        assert_eq!(r, Tdk::Success);
        assert_eq!(d.first_undeduced(), Some(1));
        let tmpl = two_param_template(s);
        assert_eq!(
            note_text(s, sink, tmpl, Tdk::Incomplete, &d),
            ["note: candidate template ignored: couldn't infer template argument 'U'"]
        );
    });
}
