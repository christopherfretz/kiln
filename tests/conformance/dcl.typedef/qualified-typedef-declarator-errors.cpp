// [dcl.typedef]/1, [dcl.meaning.general]/1: a typedef declarator cannot be
// qualified; [dcl.type.elab]/2: an elaborated-type-specifier cannot name a
// typedef-name. Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ args: -std=c++20
//@ diag: 12:16 error "typedef declarator cannot be qualified"
//@ diag: 13:16 error "typedef declarator cannot be qualified"
//@ diag: 16:6 error "typedef 'TE' cannot be referenced with the 'enum' specifier"
//@ diag: 15:12 note "declared here"
struct Q { typedef int J; };
typedef int Q::K;
typedef int Q::J;
enum E0 { e0 };
typedef E0 TE;
enum TE ee;
// Valid: a typedef-name as the nested-name-specifier of a class
// definition, a typedef declared in its class, and an alias-declaration.
struct S { struct N; };
typedef S T;
struct T::N {};
struct R { typedef int M; using O = int; };
R::M m = 0;
