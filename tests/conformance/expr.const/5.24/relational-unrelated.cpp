// [expr.const]/5.24, [expr.rel]/4: a relational comparison of pointers to unrelated objects.
// Mirrors crates/kiln_oven/tests/expr_const.rs `expr_const_5_24_relational_unrelated`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:15 error "static assertion expression is not an integral constant expression"
//@ diag: 8:18 note "comparison between '&a' and '&b' has unspecified value"
constexpr int a = 1, b = 2;
static_assert(&a < &b);
