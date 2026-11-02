// [expr.add]/5.3: subtracting pointers into different arrays.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_ptr_sub_cross_array`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:15 error "static assertion expression is not an integral constant expression"
//@ diag: 13:16 note "arithmetic involving unrelated objects '&b[1]' and '&a[0]' has unspecified value"
//@ diag: 15:15 note "in call to 'f()'"
//@ requires: clang>=22
// kiln at -fclang-compat=22 (WP-377); clang 19 wording: "subexpression not valid in a constant expression"
constexpr long f() {
  int a[2] = {1, 2};
  int b[2] = {3, 4};
  return &b[1] - &a[0];
}
static_assert(f() == 0);
