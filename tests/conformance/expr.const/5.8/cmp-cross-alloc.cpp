// [expr.rel]/4.3, [expr.const]/5.24: relational comparison of unrelated objects is unspecified.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_cmp_cross_alloc`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:15 error "static assertion expression is not an integral constant expression"
//@ diag: 13:13 note "comparison between '&a' and '&b' has unspecified value"
//@ diag: 15:15 note "in call to 'f()'"
//@ requires: clang<22
// clang 22 emits error: static assertion expression is not an integral constant expression; WP-355 residual
constexpr bool f() {
  int a = 1;
  int b = 2;
  return &a < &b;
}
static_assert(f());
