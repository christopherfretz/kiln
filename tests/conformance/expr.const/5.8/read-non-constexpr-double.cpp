// [expr.const]/5.9, [expr.const]/4: a const double is not usable in constant expressions.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_read_non_constexpr_double`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:15 error "static assertion expression is not an integral constant expression"
//@ diag: 9:15 note "read of non-constexpr variable 'dd' is not allowed in a constant expression"
//@ diag: 8:14 note "declared here"
const double dd = 1.0;
static_assert(dd == 1.0);
