// [expr.rel]/4.3, [expr.const]/5.24: relational comparison of unrelated objects is unspecified.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_cmp_cross_alloc`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:15 error "static assertion expression is not an integral constant expression"
//@ diag: 13:13 note "'&a' and '&b' has unspecified value"
//@ diag: 15:15 note "in call to 'f()'"
// clang 19 and 22 word these differently: the substrings above are common
// to both, and kiln follows its -fclang-compat level (WP-377).
constexpr bool f() {
  int a = 1;
  int b = 2;
  return &a < &b;
}
static_assert(f());
