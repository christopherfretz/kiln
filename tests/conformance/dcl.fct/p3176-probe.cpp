// [dcl.fct]/3, [depr.ellipsis.comma] (C++26, P3176R1): a variadic ellipsis
// not preceded by a comma is deprecated. WP-185 feature probe.
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang>=22
// clang 19 lacks P3176 (the warning is in clang 20).
//@ diag: 8:11 warning "declaration of a variadic function without a comma before '...' is deprecated"
void f(int...);
void g(int, ...);
