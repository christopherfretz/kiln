// [expr.mul]/4: integer division by zero.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_div_zero`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:15 error "static assertion expression is not an integral constant expression"
//@ diag: 8:42 note "division by zero"
//@ diag: 9:15 note "in call to 'f(1, 0)'"
constexpr int f(int a, int b) { return a / b; }
static_assert(f(1, 0) == 0);
