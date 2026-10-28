// [expr.mul]/4: floating division by zero.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_float_div_zero`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:15 error "static assertion expression is not an integral constant expression"
//@ diag: 8:51 note "division by zero"
//@ diag: 9:15 note "in call to 'f(1.000000e+00, 0.000000e+00)'"
constexpr double f(double a, double b) { return a / b; }
static_assert(f(1.0, 0.0) == 0);
