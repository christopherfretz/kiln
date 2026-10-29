// [expr.new]/8: a constant bound exceeding the address space is ill-formed.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 5:23 error "array is too large (4611686018427387903 elements)"
void f() { new int[2][0x3fffffffffffffff]; }
