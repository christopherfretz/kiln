// [expr.const]/5.24, [expr.rel]/4: a relational comparison of pointers to unrelated objects.
// Mirrors crates/kiln_oven/tests/expr_const.rs `expr_const_5_24_relational_unrelated`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:15 error "static assertion expression is not an integral constant expression"
//@ diag: 10:18 note "'&a' and '&b' has unspecified value"
// clang 19 and 22 word these differently: the substrings above are common
// to both, and kiln follows its -fclang-compat level (WP-377).
constexpr int a = 1, b = 2;
static_assert(&a < &b);
