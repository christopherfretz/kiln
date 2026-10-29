//! Per-rule tests for class definitions and member declarations ([class],
//! [class.mem], [class.derived], [class.friend], [dcl.fct.def]).
//!
//! Member functions, static data members, bases, friends, access labels
//! and default member initializers are WP-066b's sema; special members,
//! virtual functions and operators are still sema stubs that report
//! "... is not supported yet". These tests pin that the parser accepts
//! each form (only those stub diagnostics appear) and pin the parser's own
//! diagnostics against clang.

mod common;
use common::{Parsed, parse};

/// Asserts that every problem is a sema stub's "not supported yet", i.e.
/// the parser accepted every production.
#[track_caller]
fn assert_parsed(p: &Parsed) {
    let bad: Vec<_> =
        p.problems().into_iter().filter(|d| !d.contains("is not supported yet")).collect();
    assert!(bad.is_empty(), "unexpected diagnostics:\n{}", bad.join("\n"));
}

/// [class.ctor], [class.dtor], [class.conv.fct], [over.oper],
/// [dcl.fct.spec]/4: special declarator-ids and `explicit(bool)`.
#[test]
fn class_special_declarator_ids() {
    let p = parse(
        "struct S { ~S(); operator int(); int operator()(int); int operator[](int, int);
           explicit(false) S(int); explicit S(char); operator const int *(); };",
    );
    assert_parsed(&p);
    p.assert_diag("1:12 error: destructors is not supported yet");
    p.assert_diag("1:18 error: conversion functions is not supported yet");
    p.assert_diag("1:38 error: overloaded operators is not supported yet");
    p.assert_diag("2:28 error: constructors is not supported yet");
}

/// [class.friend]: friend declarations with special declarator-ids reach
/// sema's stubs by name.
#[test]
fn class_friend_special_names() {
    let p = parse(
        "struct P { friend P operator<(P, int); friend constexpr P operator<=>(P v, int) noexcept \
         { return v; } friend P::operator int(); };",
    );
    assert_parsed(&p);
    p.assert_diag("1:21 error: overloaded operators is not supported yet");
}

/// [over.literal]: literal operators are not supported.
#[test]
fn class_literal_operator_unsupported() {
    let p = parse("struct S { int operator\"\"_x(unsigned long long); };");
    p.assert_diag("literal operators is not supported yet");
}

/// [dcl.constexpr], [dcl.constinit]: the specifiers on members, and that
/// at most one of constexpr/consteval/constinit appears.
#[test]
fn class_constexpr_specifiers() {
    let p = parse(
        "struct S { consteval int f(); constexpr static int v = 1; constinit static int w; };",
    );
    assert_parsed(&p);
    p.assert_diag("1:12 error: 'consteval' is not supported yet");
    let p = parse("struct S { consteval constexpr int m(); };");
    p.assert_diag("1:22 error: cannot combine with previous 'consteval' declaration specifier");
}

/// [dcl.fct]/3, [class.mfct.non.static]: cv- and ref-qualifiers,
/// duplicated ones diagnosed.
#[test]
fn class_member_qualifiers() {
    let p = parse("struct S { void f() const volatile &&; void g() &; };");
    assert_parsed(&p);
    let p = parse("struct S { void q() const const; };");
    p.assert_diag("1:27 warning: duplicate 'const' declaration specifier");
    let p = parse("struct S { void r() & &&; };");
    p.assert_diag("1:22 error: expected ';' at end of declaration list");
}

/// [dcl.fct]/6: an explicit object parameter.
#[test]
fn class_explicit_object_parameter() {
    let p = parse("struct S { void s(this S self); void t(this auto &&); };");
    assert_parsed(&p);
    p.assert_diag("explicit object parameters is not supported yet");
}

/// [class.mem.general]: a member declarator has no direct-initializer, so
/// `(` starts a parameter list.
#[test]
fn class_member_paren_is_function() {
    let p = parse("struct S { int c(3); };");
    p.assert_diag("1:18 error: expected parameter declarator");
}

/// [class.virtual], [class.abstract]: virt-specifiers and pure-specifiers.
#[test]
fn class_virt_and_pure_specifiers() {
    let p = parse(
        "struct B { virtual void g(); }; struct S : B { virtual void f() = 0; virtual void g() override final; };",
    );
    assert_parsed(&p);
    let p = parse("struct S { virtual void g() override final override; };");
    p.assert_diag("1:44 error: class member already marked 'override'");
    let p = parse("struct S { void h() = 1; };");
    p.assert_diag("1:17 error: initializer on function does not look like a pure-specifier");
    let p = parse("struct S { int o : 3 final; };");
    p.assert_diag("1:21 error: expected ';' at end of declaration list");
    let p = parse("struct S { friend void p() final; };");
    p.assert_diag("1:28 error: 'final' is invalid in friend declarations");
}

/// [class.pre]/3: `final` on a class; a repeated `final`; `final` as a
/// class name.
#[test]
fn class_final() {
    let p = parse("struct A final {}; struct final { int y; };");
    p.assert_clean();
    p.assert_dump("FinalAttr");
    let p = parse("struct S final final {};");
    p.assert_diag("1:16 error: class already marked 'final'");
}

/// [class.bit]: bit-fields with default member initializers, a named
/// enumeration bit-field, and the anonymous bit-field initializer error.
#[test]
fn class_bit_fields() {
    let p = parse("struct S { enum G : int { g0 } : 3; unsigned u : 2 = 1, v : 3 {2}; };");
    p.assert_clean();
    let p = parse("struct S { int : 3 = 1; };");
    p.assert_diag("1:20 error: anonymous bit-field cannot have a default member initializer");
    let p = parse("struct S { int a : ; };");
    p.assert_diag("1:20 error: expected expression");
}

/// [dcl.attr.nouniqueaddr], [dcl.stc]/9: `[[no_unique_address]]` and
/// `mutable` members.
#[test]
fn class_member_attrs_and_mutable() {
    let p = parse("struct E {}; struct S { [[no_unique_address]] E e; mutable int m; int v; };");
    p.assert_clean();
}

/// [class.friend]/2: a friend declaration cannot define a class.
#[test]
fn class_friend_defines_type() {
    let p = parse("struct S { friend struct T { int x; }; };");
    p.assert_diag("1:28 error: cannot define a type in a friend declaration");
}

/// [class.access.spec]: a malformed access label.
#[test]
fn class_access_label_recovery() {
    let p = parse("struct S { int a; private int b; };");
    p.assert_diag("1:26 error: expected ':'");
    let p = parse("struct S { int a public: int b; };");
    p.assert_diag("1:17 error: expected ';' at end of declaration list");
}

/// [dcl.fct.def.default], [dcl.fct.def.delete]: `= default` / `= delete`
/// on non-functions and in multi-declarator declarations, and
/// `= delete("reason")` (P2573).
#[test]
fn class_defaulted_deleted() {
    let p = parse("int x = default;");
    p.assert_diag(
        "1:9 error: only special member functions and comparison operators may be defaulted",
    );
    let p = parse("int z = delete;");
    p.assert_diag("1:9 error: only functions can have deleted definitions");
    let p = parse("void zz(), zy() = delete;");
    p.assert_diag(
        "1:19 error: '= delete' is a function definition and must occur in a standalone declaration",
    );
    let p = parse("struct S { void f() = delete(\"why\"); void g() = delete(\"a\" \"b\"); };");
    assert_parsed(&p);
    let p = parse("struct S { void l() = delete(1); };");
    p.assert_diag("1:30 error: expected string literal in 'delete'");
}

/// [class.base.init]: constructor initializers (delegating, packs) parse;
/// a malformed one is diagnosed.
#[test]
fn class_ctor_initializers() {
    let p = parse("struct S { S() : S(1) {} S(int); int a; S(char) : a{1} {} };");
    assert_parsed(&p);
    let p = parse("struct S { S() : a(1) b(2) {} int a, b; };");
    p.assert_diag("1:23 error: expected '{' or ','");
}

/// [class.mfct]/2: out-of-line member definitions.
#[test]
fn class_out_of_line_definitions() {
    let p = parse("struct S { int a; S(); void f(); }; S::S() : a(1) {} void S::f() {}");
    assert_parsed(&p);
    p.assert_diag("1:19 error: constructors is not supported yet");
    p.assert_diag("1:40 error: constructors is not supported yet");
}

/// [expr.prim.this]: `this` reaches sema (outside a member function).
#[test]
fn expr_prim_this() {
    let p = parse("int x = this;");
    p.assert_diag("1:9 error: invalid use of 'this' outside of a non-static member function");
}

/// [expr.ref]/1: a qualified-id after `.`/`->` is reported unsupported
/// (no cascade of parse errors).
#[test]
fn expr_ref_qualified_member() {
    let p = parse("struct B { int x; }; struct D : B {}; int f(D d) { return d.B::x; }");
    p.assert_diag("1:61 error: qualified member access is not supported yet");
    assert_eq!(p.problems().len(), 1, "{:?}", p.problems());
}

/// [class.pre], [dcl.enum]: a missing `;` after a definition is diagnosed
/// at the next token's line, and parsing continues.
#[test]
fn class_missing_semi() {
    for (src, what) in [
        ("struct S { int x; }\nint a;", "struct"),
        ("class C { int x; }\nint a;", "class"),
        ("union U { int i; }\nint a;", "union"),
        ("enum E { A }\nint a;", "enum"),
    ] {
        let p = parse(src);
        p.assert_diag(&format!("error: expected ';' after {what}"));
        p.assert_dump("VarDecl a 'int'");
    }
    let p = parse("struct S { int a; } struct T {};");
    p.assert_diag("error: expected ';' after struct");
}

/// [class.mem.general]: an unterminated class definition is diagnosed and
/// does not panic, with members' saved contexts dropped cleanly.
#[test]
fn class_unterminated() {
    let p = parse("struct S {\n  int a = 1;\n  int f(int = 2) noexcept(true) { return a; }\n");
    p.assert_diag("error: expected '}'");
    p.assert_diag("note: to match this '{'");
}

/// [class.mem.general]/7: default member initializers are replayed once
/// the class is complete, and reported there.
#[test]
fn class_nsdmi_deferred() {
    let p = parse("struct S { int x = sizeof(S); };");
    p.assert_clean();
    let p = parse("struct S { int x = y; };");
    p.assert_diag("1:20 error: use of undeclared identifier 'y'");
}
