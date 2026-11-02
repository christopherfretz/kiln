// [expr.add]/5: subtracting a non-null pointer from null.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_ptr_sub_null`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:15 error "static assertion expression is not an integral constant expression"
//@ diag: 13:12 note "subexpression not valid in a constant expression"
//@ diag: 15:15 note "in call to 'f()'"
//@ requires: clang<22
// clang 22 emits error: static assertion expression is not an integral constant expression; WP-355 residual
constexpr long f() {
  int x[2] = {1, 2};
  int *p = nullptr;
  return p - x;
}
static_assert(f() == 0);
