// [expr.const]/5.9, [expr.const]/4: a const variable whose initializer is not a constant expression.
// Mirrors crates/kiln_oven/tests/expr_const.rs `expr_const_5_9_non_constant_initializer`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:15 error "static assertion expression is not an integral constant expression"
//@ diag: 10:15 note "initializer of 'k' is not a constant expression"
//@ diag: 9:11 note "declared here"
int h() { return 1; }
const int k = h();
static_assert(k == 1);
