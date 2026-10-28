// [expr.add]/5: subtracting a non-null pointer from null.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_ptr_sub_null`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:15 error "static assertion expression is not an integral constant expression"
//@ diag: 11:12 note "subexpression not valid in a constant expression"
//@ diag: 13:15 note "in call to 'f()'"
constexpr long f() {
  int x[2] = {1, 2};
  int *p = nullptr;
  return p - x;
}
static_assert(f() == 0);
