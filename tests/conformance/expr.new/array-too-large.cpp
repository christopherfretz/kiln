// [expr.new]/8: a constant bound exceeding the address space is ill-formed.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:23 error "array is too large (4611686018427387903 elements)"
//@ requires: clang<22
// clang 19 spelling; array-too-large-clang22.cpp has clang 22's (WP-377).
void f() { new int[2][0x3fffffffffffffff]; }
