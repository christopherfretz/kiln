// [expr.const]/5.3: an invocation of a non-constexpr function.
// Mirrors crates/kiln_oven/tests/expr_const.rs `expr_const_5_3_non_constexpr_function`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:15 error "static assertion expression is not an integral constant expression"
//@ diag: 10:28 note "non-constexpr function 'nc' cannot be used in a constant expression"
//@ diag: 11:15 note "in call to 'f()'"
//@ diag: 9:5 note "declared here"
int nc() { return 1; }
constexpr int f() { return nc(); }
static_assert(f() == 1);
