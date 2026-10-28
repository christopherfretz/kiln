//! The diagnostics for a failed implicit conversion, per context
//! ([dcl.init.general]/16.9, [expr.assign]/3, [stmt.return]/2,
//! [conv.general]/4, [dcl.init.ref]/5, [conv.ptr]/3, [conv.mem]/2).
//! Each expectation is clang 19's message (x86_64-linux-gnu,
//! -std=c++26) for the C++ in the test's comment.

#![allow(clippy::many_single_char_names, reason = "short names mirror the C++ under test")]

mod conv_common;

use conv_common::*;
use kiln_ast::expr::{ExprFlags, UnaryOp};
use kiln_ast::{ExprId, QualType};
use kiln_diag::MemorySink;
use kiln_sema::Sema;
use kiln_sema::conv::{ConvCtx, contextually_convert_to_bool, implicit_convert};

/// The error converting `e` to `to` in `ctx`, as printed.
fn fails(s: &mut Sema<'_, '_>, sink: &MemorySink, e: ExprId, to: QualType, ctx: ConvCtx) -> String {
    let err = implicit_convert(s, e, to, ctx).expect_err("conversion must fail");
    message(s, sink, &err, e)
}

fn deref(s: &mut Sema<'_, '_>, p: ExprId, ty: QualType) -> ExprId {
    s.ast.builder().unary(UnaryOp::Deref, p, ty, L, ExprFlags::NONE)
}

fn addr_of(s: &mut Sema<'_, '_>, e: ExprId) -> ExprId {
    let t = s.ast.exprs.get(e).ty();
    let pt = ptr(s, t);
    s.ast.builder().unary(UnaryOp::AddrOf, e, pt, L, ExprFlags::NONE)
}

/// [dcl.init.general]/16.9: no standard conversion from an arithmetic
/// lvalue to a pointer, or back. `int* p1 = i; int* p2 = d; int a1 = p;`
#[test]
fn dcl_init_general_16_9_arithmetic_pointer_mismatch() {
    with_sema(|s, sink| {
        let ip = ptr(s, QualType::INT);
        let i = lvalue(s, "i", QualType::INT);
        assert_eq!(
            fails(s, sink, i, ip, ConvCtx::Initialization),
            "error: cannot initialize a variable of type 'int *' with an lvalue of type 'int'"
        );
        let d = lvalue(s, "d", QualType::DOUBLE);
        assert_eq!(
            fails(s, sink, d, ip, ConvCtx::Initialization),
            "error: cannot initialize a variable of type 'int *' with an lvalue of type 'double'"
        );
        let p = lvalue(s, "p", ip);
        assert_eq!(
            fails(s, sink, p, QualType::INT, ConvCtx::Initialization),
            "error: cannot initialize a variable of type 'int' with an lvalue of type 'int *'"
        );
    });
}

/// [conv.qual]/3: initialization cannot drop qualifiers, at the top
/// level or nested. `int* p3 = cp; const int** q = pp;`
#[test]
fn conv_qual_3_initialization_drops_qualifiers() {
    with_sema(|s, sink| {
        let ip = ptr(s, QualType::INT);
        let ci = cnst(s, QualType::INT);
        let cip = ptr(s, ci);
        let cp = lvalue(s, "cp", cip);
        assert_eq!(
            fails(s, sink, cp, ip, ConvCtx::Initialization),
            "error: cannot initialize a variable of type 'int *' with an lvalue of type 'const int *'"
        );
        let ipp = ptr(s, ip);
        let cipp = ptr(s, cip);
        let pp = lvalue(s, "pp", ipp);
        assert_eq!(
            fails(s, sink, pp, cipp, ConvCtx::Initialization),
            "error: cannot initialize a variable of type 'const int **' with an lvalue of type 'int **'"
        );
    });
}

/// [stmt.return]/2, [class.copy.elision]/3: the implicitly movable
/// operand is an xvalue and prints as an rvalue.
/// `int* rp() { double d = 0; return d; }`
#[test]
fn stmt_return_2_return_object() {
    with_sema(|s, sink| {
        let ip = ptr(s, QualType::INT);
        let d = xvalue(s, "d", QualType::DOUBLE);
        assert_eq!(
            fails(s, sink, d, ip, ConvCtx::Return),
            "error: cannot initialize return object of type 'int *' with an rvalue of type 'double'"
        );
    });
}

/// [dcl.init.general]/16.7: a class source with no conversion functions.
/// `struct A {}; int x = a; i = a;`
#[test]
fn dcl_init_general_16_7_class_without_conversion_functions() {
    with_sema(|s, sink| {
        let a_t = class(s, "A", &[]);
        let a = lvalue(s, "a", a_t);
        assert_eq!(
            fails(s, sink, a, QualType::INT, ConvCtx::Initialization),
            "error: no viable conversion from 'A' to 'int'"
        );
        let a2 = lvalue(s, "a2", a_t);
        assert_eq!(
            fails(s, sink, a2, QualType::INT, ConvCtx::Assignment),
            "error: assigning to 'int' from incompatible type 'A'"
        );
    });
}

/// [expr.assign]/3: integer/pointer mismatches, with clang's fix-it
/// hints. `p = i; i = p; p = *p; i = &i;`
#[test]
fn expr_assign_3_integer_pointer_mismatch() {
    with_sema(|s, sink| {
        let ip = ptr(s, QualType::INT);
        let i = lvalue(s, "i", QualType::INT);
        assert_eq!(
            fails(s, sink, i, ip, ConvCtx::Assignment),
            "error: incompatible integer to pointer conversion assigning to 'int *' from 'int'; take the address with &"
        );
        let p = lvalue(s, "p", ip);
        assert_eq!(
            fails(s, sink, p, QualType::INT, ConvCtx::Assignment),
            "error: incompatible pointer to integer conversion assigning to 'int' from 'int *'; dereference with *"
        );
        let p2 = lvalue(s, "p2", ip);
        let dp = deref(s, p2, QualType::INT);
        assert_eq!(
            fails(s, sink, dp, ip, ConvCtx::Assignment),
            "error: incompatible integer to pointer conversion assigning to 'int *' from 'int'; remove *"
        );
        let i2 = lvalue(s, "i2", QualType::INT);
        let ai = addr_of(s, i2);
        assert_eq!(
            fails(s, sink, ai, QualType::INT, ConvCtx::Assignment),
            "error: incompatible pointer to integer conversion assigning to 'int' from 'int *'; remove &"
        );
    });
}

/// [expr.assign]/3 with [conv.qual]/3 and [conv.ptr]: pointer
/// mismatches. `p = cp; q2 = pp; lp = p;`
#[test]
fn expr_assign_3_pointer_mismatch() {
    with_sema(|s, sink| {
        let ip = ptr(s, QualType::INT);
        let ci = cnst(s, QualType::INT);
        let cip = ptr(s, ci);
        let cp = lvalue(s, "cp", cip);
        assert_eq!(
            fails(s, sink, cp, ip, ConvCtx::Assignment),
            "error: assigning to 'int *' from 'const int *' discards qualifiers"
        );
        let ipp = ptr(s, ip);
        let cipp = ptr(s, cip);
        let pp = lvalue(s, "pp", ipp);
        assert_eq!(
            fails(s, sink, pp, cipp, ConvCtx::Assignment),
            "error: assigning to 'const int **' from 'int **' discards qualifiers in nested pointer types"
        );
        let lp = ptr(s, QualType::LONG);
        let p = lvalue(s, "p", ip);
        assert_eq!(
            fails(s, sink, p, lp, ConvCtx::Assignment),
            "error: incompatible pointer types assigning to 'long *' from 'int *'"
        );
    });
}

/// [conv.general]/4, [conv.bool]: a scoped enumeration is not
/// contextually convertible to `bool`. `enum class S { s0 }; if (s) {}`
#[test]
fn conv_general_4_scoped_enum_condition() {
    with_sema(|s, sink| {
        let st = enumeration(s, "S", true, Some(QualType::INT), (QualType::INT, QualType::INT));
        let e = lvalue(s, "s", st);
        let err = contextually_convert_to_bool(s, e).expect_err("not convertible");
        assert_eq!(
            message(s, sink, &err, e),
            "error: value of type 'S' is not contextually convertible to 'bool'"
        );
    });
}

/// [dcl.init.ref]/5.2: a non-const lvalue reference binds neither an
/// unrelated lvalue nor a temporary. `int& r1 = d; int& r2 = 5;
/// volatile int& r6 = 5;`
#[test]
fn dcl_init_ref_5_2_non_const_lvalue_reference() {
    with_sema(|s, sink| {
        let ir = lref(s, QualType::INT);
        let d = lvalue(s, "d", QualType::DOUBLE);
        assert_eq!(
            fails(s, sink, d, ir, ConvCtx::Initialization),
            "error: non-const lvalue reference to type 'int' cannot bind to a value of unrelated type 'double'"
        );
        let five = int_lit(s, QualType::INT, 5);
        assert_eq!(
            fails(s, sink, five, ir, ConvCtx::Initialization),
            "error: non-const lvalue reference to type 'int' cannot bind to a temporary of type 'int'"
        );
        let vi = vol(s, QualType::INT);
        let vir = lref(s, vi);
        let five = int_lit(s, QualType::INT, 5);
        assert_eq!(
            fails(s, sink, five, vir, ConvCtx::Initialization),
            "error: volatile lvalue reference to type 'volatile int' cannot bind to a temporary of type 'int'"
        );
    });
}

/// [dcl.init.ref]/5.4.4: an rvalue reference does not bind an lvalue of
/// a reference-related type. `int&& r3 = i;`
#[test]
fn dcl_init_ref_5_4_4_rvalue_reference_to_lvalue() {
    with_sema(|s, sink| {
        let irr = rref(s, QualType::INT);
        let i = lvalue(s, "i", QualType::INT);
        assert_eq!(
            fails(s, sink, i, irr, ConvCtx::Initialization),
            "error: rvalue reference to type 'int' cannot bind to lvalue of type 'int'"
        );
    });
}

/// [dcl.init.ref]/5: binding needs the referee at least as cv-qualified
/// ([dcl.init.ref]/4, reference-compatible). `int& r4 = cp[0];
/// int& r5 = cvi; volatile int& r7 = ci;`
#[test]
fn dcl_init_ref_5_drops_qualifiers() {
    with_sema(|s, sink| {
        let ir = lref(s, QualType::INT);
        let ci = cnst(s, QualType::INT);
        let e = lvalue(s, "c", ci);
        assert_eq!(
            fails(s, sink, e, ir, ConvCtx::Initialization),
            "error: binding reference of type 'int' to value of type 'const int' drops 'const' qualifier"
        );
        let cvi = vol(s, ci);
        let e = lvalue(s, "cvi", cvi);
        assert_eq!(
            fails(s, sink, e, ir, ConvCtx::Initialization),
            "error: binding reference of type 'int' to value of type 'const volatile int' drops 'const volatile' qualifiers"
        );
        let vi = vol(s, QualType::INT);
        let vir = lref(s, vi);
        let e = lvalue(s, "ci", ci);
        assert_eq!(
            fails(s, sink, e, vir, ConvCtx::Initialization),
            "error: binding reference of type 'volatile int' to value of type 'const int' drops 'const' qualifier"
        );
    });
}

const AMBIGUOUS_D_TO_A: &str = "error: ambiguous conversion from derived class 'D' to base class 'A':\n    struct D -> B -> A\n    struct D -> C -> A";

/// [conv.ptr]/3, [dcl.init.ref]/5 with [class.member.lookup]: an
/// ambiguous base. `struct D : B, C {}; A* ap = dp; A& ar = dd;`
#[test]
fn conv_ptr_3_ambiguous_base() {
    with_sema(|s, sink| {
        let a = class(s, "A", &[]);
        let b = class(s, "B", &[(a, false)]);
        let c = class(s, "C", &[(a, false)]);
        let d = class(s, "D", &[(b, false), (c, false)]);
        let ap = ptr(s, a);
        let dpt = ptr(s, d);
        let dp = lvalue(s, "dp", dpt);
        assert_eq!(fails(s, sink, dp, ap, ConvCtx::Initialization), AMBIGUOUS_D_TO_A);
        let ar = lref(s, a);
        let dd = lvalue(s, "dd", d);
        assert_eq!(fails(s, sink, dd, ar, ConvCtx::Initialization), AMBIGUOUS_D_TO_A);
    });
}

/// [conv.mem]/2: no conversion through a virtual or ambiguous base.
/// `struct X : virtual V {}; int X::* xm = vm; int D::* dm = am;`
#[test]
fn conv_mem_2_virtual_and_ambiguous_base() {
    with_sema(|s, sink| {
        let v = class(s, "V", &[]);
        let x = class(s, "X", &[(v, true)]);
        let vm_t = mem_ptr(s, QualType::INT, v);
        let xm_t = mem_ptr(s, QualType::INT, x);
        let vm = lvalue(s, "vm", vm_t);
        assert_eq!(
            fails(s, sink, vm, xm_t, ConvCtx::Initialization),
            "error: conversion from pointer to member of class 'V' to pointer to member of class 'X' via virtual base 'V' is not allowed"
        );
        let a = class(s, "A", &[]);
        let b = class(s, "B", &[(a, false)]);
        let c = class(s, "C", &[(a, false)]);
        let d = class(s, "D", &[(b, false), (c, false)]);
        let am_t = mem_ptr(s, QualType::INT, a);
        let dm_t = mem_ptr(s, QualType::INT, d);
        let am = lvalue(s, "am", am_t);
        assert_eq!(
            fails(s, sink, am, dm_t, ConvCtx::Initialization),
            "error: ambiguous conversion from pointer to member of base class 'A' to pointer to member of derived class 'D':\n    struct D -> B -> A\n    struct D -> C -> A"
        );
    });
}
