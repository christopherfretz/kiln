// [expr.add]/4 with [basic.compound]/3: a non-array object is an array of one element.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_oob_non_array`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:15 error "static assertion expression is not an integral constant expression"
//@ diag: 10:15 note "cannot refer to element 2 of non-array object in a constant expression"
//@ diag: 13:15 note "in call to 'f()'"
constexpr int f() {
  int x = 1;
  int *p = &x + 2;
  return 0;
}
static_assert(f() == 0);
