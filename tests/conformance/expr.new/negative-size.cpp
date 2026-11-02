// [expr.new]/8: a negative constant bound is ill-formed.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:20 error "array is too large (18446744073709551615 elements)"
//@ requires: clang<22
// clang 22 emits error: array is too large (18'446'744'073'709'551'615 elements); WP-355 residual
void f() { new int[-1]; }
