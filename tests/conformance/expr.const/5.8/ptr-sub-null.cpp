// [expr.add]/5: subtracting a non-null pointer from null.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_ptr_sub_null`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:15 error "static assertion expression is not an integral constant expression"
//@ diag: 13:12 note "arithmetic involving unrelated objects 'nullptr' and '&x[0]' has unspecified value"
//@ diag: 15:15 note "in call to 'f()'"
//@ requires: clang>=22
// kiln at -fclang-compat=22 (WP-377); clang 19 wording: "subexpression not valid in a constant expression"
constexpr long f() {
  int x[2] = {1, 2};
  int *p = nullptr;
  return p - x;
}
static_assert(f() == 0);
