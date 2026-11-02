// [namespace.udecl]/3: in a class, the nested-name-specifier names a base
// class; E names D, which is not a base of E (line 13)
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:25 error "using declaration in class refers into 'N"
//@ diag: 14:37 error "using declaration refers to its own class"
//@ diag: 14:49 error "using declaration in class refers into '::N"
//@ diag: 15:18 error "', which is not a base class of 'E'"
//@ diag: 16:18 error "using declaration refers to its own class"
// clang 19 names the specifier ('N::') and also reports "no member named 'q'
// in 'F'" at 16:21; clang 22 names the scope ('N') and stops (WP-377).
//@ requires: clang>=22
namespace N { int x; }
struct D { int a; using N::x; using D::a; using ::N::x; };
struct E { using D::a; };
struct F { using F::q; };
