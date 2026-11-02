// [expr.new]/8: a negative constant bound is ill-formed.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:20 error "array is too large (18'446'744'073'709'551'615 elements)"
//@ requires: clang>=22
// clang 22 spelling of negative-size.cpp (WP-377).
void f() { new int[-1]; }
