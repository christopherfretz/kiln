// [dcl.fct]/17: types shall not be defined in parameter types. In the
// parameter clause of an out-of-line member definition the unnamed
// enumeration used to be entered into the (complete) class, and kiln
// panicked with "member added to a complete class" (kiln_ast append_member;
// fixed by WP-962). clang prints '(null)' for the unnamed enumeration. Found
// by the clang negative-test run (WP-912, dcl.fct/p17 family).
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:11 error "(null) cannot be defined in a parameter type"
//@ diag: 15:6 error "(null) cannot be defined in a parameter type"
struct A {};
struct B {};
void A::f(enum { e2 }) {}
B::B(enum { e1 }) {}
