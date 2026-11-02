// [expr.new]/8: a constant bound exceeding the address space is ill-formed.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:23 error "array is too large (4611686018427387903 elements)"
//@ requires: clang<22
// clang 22 emits error: array is too large (4'611'686'018'427'387'903 elements); WP-355 residual
void f() { new int[2][0x3fffffffffffffff]; }
