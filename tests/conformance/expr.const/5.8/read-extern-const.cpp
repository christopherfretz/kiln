// [expr.const]/4: a const int without a preceding initializer is not usable.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_read_extern_const`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:15 error "static assertion expression is not an integral constant expression"
//@ diag: 9:15 note "initializer of 'e' is unknown"
//@ diag: 8:18 note "declared here"
extern const int e;
static_assert(e == 1);
