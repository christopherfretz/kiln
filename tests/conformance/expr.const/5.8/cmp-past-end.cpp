// [expr.eq]/3.1, [expr.const]/5.24: equality with a past-the-end pointer of another object is unspecified.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_cmp_past_end`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:15 error "static assertion expression is not an integral constant expression"
//@ diag: 11:16 note "comparison against pointer '&a[2]' that points past the end of a complete object has unspecified value"
//@ diag: 13:16 note "in call to 'f()'"
constexpr bool f() {
  int a[2] = {1, 2};
  int b = 3;
  return &a[2] == &b;
}
static_assert(!f());
