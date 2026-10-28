// [expr.add]/4.3: forming a pointer two past the end is undefined.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_oob_subscript`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:15 error "static assertion expression is not an integral constant expression"
//@ diag: 10:10 note "cannot refer to element 4 of array of 3 elements in a constant expression"
//@ diag: 12:15 note "in call to 'f(4)'"
constexpr int f(int i) {
  int a[3] = {1, 2, 3};
  return a[i];
}
static_assert(f(4) == 0);
