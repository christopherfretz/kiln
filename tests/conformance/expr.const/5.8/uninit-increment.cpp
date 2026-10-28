// [basic.indet]/2: incrementing an indeterminate value reads it.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_uninit_increment`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:15 error "static assertion expression is not an integral constant expression"
//@ diag: 10:4 note "increment of uninitialized object is not allowed in a constant expression"
//@ diag: 13:15 note "in call to 'f()'"
constexpr int f() {
  int x;
  x++;
  return x;
}
static_assert(f() == 0);
