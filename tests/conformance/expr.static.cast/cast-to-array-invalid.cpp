// [expr.static.cast]/4: an array that cannot be initialized from the
// operand is no valid static_cast (WP-1497).
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 8:16 error "static_cast from 'P' to 'int[2]' is not allowed"
struct P {};
int (&&r)[2] = static_cast<int[2]>(P{});
