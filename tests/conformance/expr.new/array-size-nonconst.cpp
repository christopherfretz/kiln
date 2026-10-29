// [expr.new]/8: every bound but the first is a converted constant expression.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:28 error "array size is not a constant expression"
//@ diag: 7:28 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 7:12 note "declared here"
void f(int n) { new int[2][n]; }
