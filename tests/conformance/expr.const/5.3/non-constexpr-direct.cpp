// [expr.const]/5.3: the non-constexpr call at the top level has no call frame.
// Mirrors crates/kiln_oven/tests/expr_const.rs `expr_const_5_3_non_constexpr_direct`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:15 error "static assertion expression is not an integral constant expression"
//@ diag: 9:15 note "non-constexpr function 'nc' cannot be used in a constant expression"
//@ diag: 8:5 note "declared here"
int nc() { return 1; }
static_assert(nc() == 1);
