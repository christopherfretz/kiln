// [expr.const]/5.9: L2R of a glvalue whose object is not usable in constant expressions and began its lifetime outside the evaluation.
// Mirrors crates/kiln_oven/tests/expr_const.rs `expr_const_5_9_non_const_global`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:15 error "static assertion expression is not an integral constant expression"
//@ diag: 10:28 note "read of non-const variable 'g' is not allowed in a constant expression"
//@ diag: 11:15 note "in call to 'f()'"
//@ diag: 9:5 note "declared here"
int g = 1;
constexpr int f() { return g; }
static_assert(f() == 1);
