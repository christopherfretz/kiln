//! WP-067a special member functions (spec/sema-class.md §4): implicit
//! declaration, deletedness, triviality, `= default` / `= delete`, the
//! constructor/destructor/conversion declarator checks and the checks on
//! default-initialized variables. Expectations follow
//! `clang++-19 -std=c++26 -fsyntax-only` (and `-Xclang -ast-dump`).

mod common;
use common::{parse, parse_implicit};

/// [class.default.ctor]/1, [class.copy.ctor]/6, [class.dtor]/2: an
/// unreferenced class declares its implicit members lazily; a class
/// member whose destructor is user-declared needs overload resolution and
/// makes them eager (spec/sema-class.md §4.1).
#[test]
fn special_implicit_members_lazy_and_eager() {
    let p = parse_implicit("struct A { int a; }; struct P1 { ~P1(); }; struct H { P1 m; };");
    p.assert_clean();
    assert!(!p.dump.contains("CXXConstructorDecl implicit constexpr A"), "{}", p.dump);
    p.assert_dump("CXXConstructorDecl implicit constexpr H 'void (const H &)' inline default trivial noexcept-unevaluated");
    p.assert_dump(
        "CXXDestructorDecl implicit constexpr ~H 'void ()' inline default noexcept-unevaluated",
    );
    // H's overload resolution declared P1's copy constructor.
    p.assert_dump(
        "CXXConstructorDecl implicit constexpr P1 'void (const P1 &)' inline default trivial",
    );
}

/// [class.copy.ctor]/11, [class.copy.assign]/9: the implicit move members
/// of `{ P1 m; }` are trivial because P1's trivial copy members are
/// selected (clang `SpecialMemberIsTrivial`).
#[test]
fn special_move_selected_copy_is_trivial() {
    let p = parse_implicit("struct P1 { ~P1(); }; struct H { P1 m; };");
    p.assert_dump("CXXConstructorDecl implicit constexpr H 'void (H &&)' inline default trivial");
    p.assert_dump("CXXMethodDecl implicit constexpr operator= 'H &(H &&)' inline default trivial");
}

/// [dcl.fct.def.default]/5, [class.default.ctor]/3: a member defaulted on
/// its first declaration is trivial when the class allows it; an NSDMI
/// makes the default constructor non-trivial; [class.dtor]/8 a virtual
/// destructor is not trivial; [dcl.constexpr] (C++23) all are `constexpr`.
#[test]
fn special_defaulted_triviality() {
    let p = parse_implicit(
        "struct T { T() = default; ~T() = default; int i; };
         struct N { N() = default; int i = 1; };
         struct V { virtual ~V() = default; };",
    );
    p.assert_clean();
    p.assert_dump("CXXConstructorDecl constexpr T 'void ()' default trivial");
    p.assert_dump("CXXDestructorDecl constexpr ~T 'void ()' default trivial");
    p.assert_dump("CXXConstructorDecl constexpr N 'void ()' default noexcept-unevaluated");
    p.assert_dump("CXXDestructorDecl constexpr ~V 'void ()' virtual default noexcept");
}

/// [class.default.ctor]/2: a reference member without an initializer
/// deletes the default constructor; [dcl.fct.def.default]/5 an explicit
/// default on the first declaration warns, out of line it is an error.
#[test]
fn special_defaulted_function_deleted() {
    let p = parse(
        "struct C { C() = default; int &r; };
         struct D { D(); int &r; };
         D::D() = default;",
    );
    p.assert_diag("1:12 warning: explicitly defaulted default constructor is implicitly deleted");
    p.assert_diag("note: default constructor of 'C' is implicitly deleted because field 'r' of reference type 'int &' would not be initialized");
    p.assert_diag("note: replace 'default' with 'delete'");
    p.assert_diag("3:13 error: defaulting this default constructor would delete it after its first declaration");
}

/// [dcl.fct.def.default]/1-2: only special members may be defaulted, with
/// the implicit declaration's type.
#[test]
fn special_defaulted_signatures() {
    let p = parse(
        "struct A { void f() = default; A &operator=(A) = default; int operator=(A &&) = default; };",
    );
    p.assert_diag(
        "1:23 error: only special member functions and comparison operators may be defaulted",
    );
    p.assert_diag(
        "error: the parameter for an explicitly-defaulted copy assignment operator must be an lvalue reference type",
    );
    p.assert_diag("error: explicitly-defaulted move assignment operator must return 'A &'");
}

/// [dcl.fct.def.delete]/2: an unqualified call of a deleted non-member
/// function reports the call (with the P2573 message); other references
/// report the use.
#[test]
fn special_deleted_uses() {
    let p = parse(
        "void f() = delete(\"gone\");
         struct S { void m() = delete; };
         void u(S s) { f(); s.m(); auto p = &f; }",
    );
    p.assert_diag("3:24 error: call to deleted function 'f': gone");
    p.assert_diag("note: candidate function has been explicitly deleted");
    p.assert_diag("3:31 error: attempt to use a deleted function");
    p.assert_diag("note: 'm' has been explicitly marked deleted here");
    p.assert_diag("3:46 error: attempt to use a deleted function: gone");
}

/// [dcl.fct.def.delete]/4: a deleted definition is the first declaration;
/// [basic.def.odr]/1 it is a definition.
#[test]
fn special_deleted_redeclarations() {
    let p = parse("void f(); void f() = delete; void g() = delete; void g() {}");
    p.assert_diag("1:22 error: deleted definition must be first declaration");
    p.assert_diag("1:54 error: redefinition of 'g'");
    p.assert_diag("note: previous definition is here");
}

/// [dcl.init.general]/7, [class.dtor]/15: a default-initialized variable
/// needs a usable default constructor and destructor.
#[test]
fn special_variable_checks() {
    let p = parse(
        "struct R { int &r; }; R r;
         struct PD { private: ~PD(); }; PD pd;
         struct DD { ~DD() = delete; }; DD dd; extern DD ok;
         struct K { int a; }; const K k;",
    );
    p.assert_diag("1:25 error: call to implicitly-deleted default constructor of 'R'");
    p.assert_diag("2:44 error: variable of type 'PD' has private destructor");
    p.assert_diag("3:44 error: attempt to use a deleted function");
    p.assert_diag(
        "4:39 error: default initialization of an object of const type 'const K' without a user-provided default constructor",
    );
    assert_eq!(p.problems().len(), 4, "{:?}", p.problems());
}

/// [class.ctor.general], [class.dtor]/1-4, [class.conv.fct]/1-4,
/// [dcl.fct.spec]/3: the declarator checks.
#[test]
fn special_declarator_checks() {
    let p = parse(
        "struct S { int S(); ~S(int); operator int(char); explicit void f(); S(S); };
         explicit int x;",
    );
    p.assert_diag("1:16 error: constructor cannot have a return type");
    p.assert_diag("1:21 error: destructor cannot have any parameters");
    p.assert_diag("1:30 error: conversion function cannot have any parameters");
    p.assert_diag(
        "1:50 error: 'explicit' can only be applied to a constructor or conversion function",
    );
    p.assert_diag("1:72 error: copy constructor must pass its first argument by reference");
    p.assert_diag("2:10 error: 'explicit' can only appear on non-static member functions");
}

/// [class.dtor]/2: `static` on a destructor is dropped without making the
/// declaration invalid, so a redeclaration is still diagnosed (clang).
#[test]
fn special_static_destructor_redeclared() {
    let p = parse("struct S { ~S(); static ~S(); };");
    p.assert_diag("1:25 error: destructor cannot be declared 'static'");
    p.assert_diag("1:25 error: destructor cannot be redeclared");
}
