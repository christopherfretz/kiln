// [expr.const]/5.9: reading a non-const variable.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_read_non_const`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:15 error "static assertion expression is not an integral constant expression"
//@ diag: 9:15 note "read of non-const variable 'g' is not allowed in a constant expression"
//@ diag: 8:5 note "declared here"
int g = 1;
static_assert(g == 1);
