// [dcl.fct]/17: types shall not be defined in parameter types (clang prints
// '(null)' for the unnamed enumeration). Found by the clang negative-test
// run (WP-912).
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:8 error "(null) cannot be defined in a parameter type"
//@ diag: 10:15 error "'S' cannot be defined in a parameter type"
void f(enum { e2 }) {}
void g(struct S { int x; } s);
