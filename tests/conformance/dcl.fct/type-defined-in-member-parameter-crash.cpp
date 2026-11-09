// [dcl.fct]/17: types shall not be defined in parameter types. In the
// parameter clause of an out-of-line member definition the unnamed
// enumeration is entered into the (complete) class, and kiln panics with
// "member added to a complete class" (kiln_ast append_member). clang prints
// '(null)' for the unnamed enumeration. Found by the clang negative-test run
// (WP-912, CXX/dcl.decl/dcl.meaning/dcl.fct/p17 family).
//@ std: c++20
//@ xfail(kiln): WP-912 follow-up, a tag defined in an out-of-line member's parameter clause panics (member added to a complete class)
//@ expect: compile-fail
//@ diag: 14:11 error "cannot be defined in a parameter type"
//@ diag: 15:6 error "cannot be defined in a parameter type"
struct A {};
struct B {};
void A::f(enum { e2 }) {}
B::B(enum { e1 }) {}
