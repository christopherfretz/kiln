// [expr.new]/8: a negative constant bound is ill-formed.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 5:20 error "array is too large (18446744073709551615 elements)"
void f() { new int[-1]; }
