//! WP-066b member access (spec/sema-class.md §3): lookup through bases
//! ([class.member.lookup]), access control ([class.access]), member
//! calls, `.*` / `->*` and the derived-to-base conversions they apply.
//! Expectations follow `clang++-19 -std=c++26 -fsyntax-only`; the parser
//! cannot spell base-clauses or access specifiers before WP-065, so the
//! classes are built directly.
#![allow(
    clippy::many_single_char_names,
    reason = "s/r/f/b/d: sema, record, function, base, derived"
)]

mod conv_common;

use conv_common::{L, int_lit, lvalue, mem_ptr, name, with_sema};
use kiln_ast::decl::{Access, BaseSpec, DeclKind, DefinitionBits, FnSpec, TagKind};
use kiln_ast::expr::{BinaryOp, ExprKind};
use kiln_ast::types::{ExcSpec, FuncInfo, Quals};
use kiln_ast::{DeclHead, DeclId, ExprId, QualType};
use kiln_diag::MemorySink;
use kiln_sema::Sema;

fn messages(sink: &MemorySink) -> Vec<String> {
    let mut out = Vec::new();
    fn walk(d: &kiln_diag::Diagnostic, out: &mut Vec<String>) {
        out.push(format!("{}: {}", d.level.text_name(), d.message));
        for n in &d.notes {
            walk(n, out);
        }
    }
    for d in sink.take() {
        walk(&d, &mut out);
    }
    out
}

/// `struct n : bases... {` with its definition begun; add members with
/// [`field`] / [`method`], then [`complete`] it.
fn record(s: &mut Sema<'_, '_>, n: &str, tag: TagKind, bases: &[(DeclId, Access)]) -> DeclId {
    let h = DeclHead::new(s.ast.tu, name(s, n), L);
    let r = s.ast.builder().record(h, tag, None);
    s.ast.builder().begin_record_definition(r).expect("first definition");
    let specs: Vec<BaseSpec> = bases
        .iter()
        .map(|&(b, access)| BaseSpec {
            ty: s.ast.decls.get(b).ty().expect("record type"),
            loc: L,
            is_virtual: false,
            access,
            pack_expansion: false,
        })
        .collect();
    let ast = &mut s.ast;
    ast.decls.set_bases(r, &specs, &mut ast.lists).expect("bases once");
    r
}

fn complete(s: &mut Sema<'_, '_>, r: DeclId) {
    s.ast.builder().complete_record_definition(r, DefinitionBits::empty());
}

fn field(s: &mut Sema<'_, '_>, r: DeclId, n: &str, ty: QualType, access: Access) -> DeclId {
    let h = DeclHead::new(r, name(s, n), L);
    let f = s.ast.builder().field(h, ty, None, 0);
    s.ast.decls.set_access(f, access);
    f
}

fn method(
    s: &mut Sema<'_, '_>,
    r: DeclId,
    n: &str,
    this_quals: Quals,
    params: &[QualType],
) -> DeclId {
    let ft = s.ast.types.function(
        QualType::INT,
        params,
        FuncInfo::new().with_this_quals(this_quals),
        ExcSpec::None,
    );
    let h = DeclHead::new(r, name(s, n), L);
    let f = s.ast.builder().function(h, DeclKind::CXXMethod, ft, FnSpec::new(), None);
    s.ast.decls.set_access(f, Access::Public);
    f
}

fn ty(s: &Sema<'_, '_>, r: DeclId) -> QualType {
    s.ast.decls.get(r).ty().expect("record type")
}

fn member(s: &mut Sema<'_, '_>, base: ExprId, n: &str, arrow: bool) -> ExprId {
    let sym = s.ast.symbols().intern(n);
    s.act_on_member(base, sym, L, L, arrow)
}

/// `e` as a pre-order list of `Kind <CastKind> 'type'` lines.
fn tree(s: &Sema<'_, '_>, e: ExprId) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![e];
    while let Some(x) = stack.pop() {
        let n = s.ast.exprs.get(x);
        let ck = if n.kind() == ExprKind::ImplicitCastExpr {
            format!(" <{:?}>", s.ast.exprs.cast_kind(x))
        } else {
            String::new()
        };
        out.push(format!("{:?}{ck} '{}'", n.kind(), s.ty_str(n.ty())));
        let mut kids = s.ast.exprs.children(x, &s.ast.lists);
        kids.reverse();
        stack.extend(kids);
    }
    out
}

#[test]
fn member_found_in_base_converts_the_object() {
    with_sema(|s, sink| {
        let b = record(s, "B", TagKind::Struct, &[]);
        field(s, b, "x", QualType::INT, Access::Public);
        complete(s, b);
        let d = record(s, "D", TagKind::Struct, &[(b, Access::Public)]);
        complete(s, d);
        let dt = ty(s, d);
        let obj = lvalue(s, "d", dt);
        let e = member(s, obj, "x", false);
        assert_eq!(messages(sink), Vec::<String>::new());
        assert_eq!(
            tree(s, e),
            [
                "MemberExpr 'int'",
                "ImplicitCastExpr <UncheckedDerivedToBase> 'B'",
                "DeclRefExpr 'D'",
            ]
        );
    });
}

#[test]
fn private_member_is_inaccessible_outside_the_class() {
    with_sema(|s, sink| {
        let a = record(s, "A", TagKind::Class, &[]);
        field(s, a, "x", QualType::INT, Access::Private);
        complete(s, a);
        let at = ty(s, a);
        let obj = lvalue(s, "a", at);
        let _ = member(s, obj, "x", false);
        assert_eq!(
            messages(sink),
            ["error: 'x' is a private member of 'A'", "note: implicitly declared private here"]
        );
    });
}

#[test]
fn member_of_private_base_is_inaccessible() {
    with_sema(|s, sink| {
        let b = record(s, "B", TagKind::Struct, &[]);
        field(s, b, "x", QualType::INT, Access::Public);
        complete(s, b);
        let d = record(s, "D", TagKind::Class, &[(b, Access::None)]);
        complete(s, d);
        let dt = ty(s, d);
        let obj = lvalue(s, "d", dt);
        let _ = member(s, obj, "x", false);
        assert_eq!(
            messages(sink),
            [
                "error: 'x' is a private member of 'B'",
                "note: constrained by implicitly private inheritance here",
                "note: member is declared here",
            ]
        );
    });
}

#[test]
fn ambiguous_member_lookup_is_diagnosed() {
    with_sema(|s, sink| {
        let b1 = record(s, "B1", TagKind::Struct, &[]);
        field(s, b1, "x", QualType::INT, Access::Public);
        complete(s, b1);
        let b2 = record(s, "B2", TagKind::Struct, &[]);
        field(s, b2, "x", QualType::INT, Access::Public);
        complete(s, b2);
        let d = record(s, "D", TagKind::Struct, &[(b1, Access::Public), (b2, Access::Public)]);
        complete(s, d);
        let dt = ty(s, d);
        let obj = lvalue(s, "d", dt);
        let e = member(s, obj, "x", false);
        assert!(s.is_error_expr(e));
        assert_eq!(
            messages(sink),
            [
                "error: member 'x' found in multiple base classes of different types",
                "note: member found by ambiguous name lookup",
                "note: member found by ambiguous name lookup",
            ]
        );
    });
}

#[test]
fn single_candidate_member_call() {
    with_sema(|s, sink| {
        let a = record(s, "A", TagKind::Struct, &[]);
        method(s, a, "f", Quals::CONST, &[QualType::INT]);
        complete(s, a);
        let at = ty(s, a);
        let obj = lvalue(s, "a", at);
        let callee = member(s, obj, "f", false);
        let one = int_lit(s, QualType::INT, 1);
        let call = s.act_on_call(callee, &[one], L, L);
        assert_eq!(messages(sink), Vec::<String>::new());
        assert_eq!(
            tree(s, call),
            [
                "CXXMemberCallExpr 'int'",
                "MemberExpr '<bound member function type>'",
                "ImplicitCastExpr <NoOp> 'const A'",
                "DeclRefExpr 'A'",
                "IntegerLiteral 'int'",
            ]
        );
    });
}

#[test]
fn member_call_arity_and_missing_call() {
    with_sema(|s, sink| {
        let a = record(s, "A", TagKind::Struct, &[]);
        method(s, a, "f", Quals::NONE, &[]);
        complete(s, a);
        let at = ty(s, a);
        let obj = lvalue(s, "a", at);
        let callee = member(s, obj, "f", false);
        let one = int_lit(s, QualType::INT, 1);
        let call = s.act_on_call(callee, &[one], L, L);
        assert!(s.is_error_expr(call));
        let m = messages(sink);
        assert_eq!(
            m.first().map(String::as_str),
            Some("error: too many arguments to function call, expected 0, have 1")
        );
        let obj = lvalue(s, "a2", at);
        let bound = member(s, obj, "f", false);
        let _ = s.act_on_expr_stmt(bound);
        assert_eq!(
            messages(sink),
            [
                "error: reference to non-static member function must be called; did you mean to call it with no arguments?"
            ]
        );
    });
}

#[test]
fn non_const_member_on_const_object() {
    with_sema(|s, sink| {
        let a = record(s, "A", TagKind::Struct, &[]);
        method(s, a, "f", Quals::NONE, &[]);
        complete(s, a);
        let at = ty(s, a);
        let ct = conv_common::cnst(s, at);
        let obj = lvalue(s, "a", ct);
        let callee = member(s, obj, "f", false);
        let _ = s.act_on_call(callee, &[], L, L);
        assert_eq!(
            messages(sink),
            [
                "error: 'this' argument to member function 'f' has type 'const A', but function \
                 is not marked const",
                "note: 'f' declared here",
            ]
        );
    });
}

#[test]
fn pointer_to_data_member_through_derived_object() {
    with_sema(|s, sink| {
        let b = record(s, "B", TagKind::Struct, &[]);
        field(s, b, "x", QualType::INT, Access::Public);
        complete(s, b);
        let d = record(s, "D", TagKind::Struct, &[(b, Access::Public)]);
        complete(s, d);
        let bt = ty(s, b);
        let dt = ty(s, d);
        let pmt = mem_ptr(s, QualType::INT, bt);
        let pm = lvalue(s, "pm", pmt);
        let obj = lvalue(s, "d", dt);
        let e = s.act_on_binary(BinaryOp::PtrMemD, obj, pm, L);
        assert_eq!(messages(sink), Vec::<String>::new());
        assert_eq!(
            tree(s, e),
            [
                "BinaryOperator 'int'",
                "ImplicitCastExpr <DerivedToBase> 'B'",
                "DeclRefExpr 'D'",
                "ImplicitCastExpr <LValueToRValue> 'int B::*'",
                "DeclRefExpr 'int B::*'",
            ]
        );
    });
}

#[test]
fn pointer_to_member_of_unrelated_class() {
    with_sema(|s, sink| {
        let a = record(s, "A", TagKind::Struct, &[]);
        complete(s, a);
        let b = record(s, "B", TagKind::Struct, &[]);
        complete(s, b);
        let at = ty(s, a);
        let bt = ty(s, b);
        let pmt = mem_ptr(s, QualType::INT, bt);
        let pm = lvalue(s, "pm", pmt);
        let obj = lvalue(s, "a", at);
        let e = s.act_on_binary(BinaryOp::PtrMemD, obj, pm, L);
        assert!(s.is_error_expr(e));
        let m = messages(sink);
        assert_eq!(m.len(), 1, "{m:?}");
    });
}
