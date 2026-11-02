// [expr.new]/8: a negative constant bound is ill-formed.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:20 error "array is too large (18446744073709551615 elements)"
//@ requires: clang<22
// clang 19 spelling; negative-size-clang22.cpp has clang 22's (WP-377).
void f() { new int[-1]; }
