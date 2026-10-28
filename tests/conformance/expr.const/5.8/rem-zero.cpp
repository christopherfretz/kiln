// [expr.mul]/4: integer remainder by zero.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_rem_zero`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:15 error "static assertion expression is not an integral constant expression"
//@ diag: 8:57 note "division by zero"
//@ diag: 9:15 note "in call to 'f(1, 0)'"
constexpr unsigned f(unsigned a, unsigned b) { return a % b; }
static_assert(f(1u, 0u) == 0);
