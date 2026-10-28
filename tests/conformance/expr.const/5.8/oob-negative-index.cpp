// [expr.add]/4.3: a negative index leaves the array.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_oob_negative_index`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:15 error "static assertion expression is not an integral constant expression"
//@ diag: 10:14 note "cannot refer to element -1 of array of 3 elements in a constant expression"
//@ diag: 12:15 note "in call to 'f(-1)'"
constexpr int f(int i) {
  int a[3] = {1, 2, 3};
  return *(a + i);
}
static_assert(f(-1) == 0);
