// [expr.rel]/4.3, [expr.const]/5.24: relational comparison of unrelated objects is unspecified.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_cmp_cross_alloc`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:15 error "static assertion expression is not an integral constant expression"
//@ diag: 11:13 note "comparison between '&a' and '&b' has unspecified value"
//@ diag: 13:15 note "in call to 'f()'"
constexpr bool f() {
  int a = 1;
  int b = 2;
  return &a < &b;
}
static_assert(f());
