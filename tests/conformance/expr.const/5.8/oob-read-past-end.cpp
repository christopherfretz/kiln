// [expr.const]/5.8, [basic.compound]/3: reading through a one-past-the-end pointer.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_oob_read_past_end`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:15 error "static assertion expression is not an integral constant expression"
//@ diag: 11:10 note "read of dereferenced one-past-the-end pointer is not allowed in a constant expression"
//@ diag: 13:15 note "in call to 'f()'"
constexpr int f() {
  int a[3] = {1, 2, 3};
  int *p = a + 3;
  return *p;
}
static_assert(f() == 0);
