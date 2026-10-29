// [basic.stc.dynamic.deallocation]/4, [expr.const]/5: a pointer to a deleted
// object is invalid; reading or writing through it is not a constant
// expression, even after the block is reused. Mirrors
// crates/kiln_oven/tests/new_delete.rs `expr_delete_faults`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:15 error "static assertion expression is not an integral constant expression"
//@ diag: 19:10 note "read of heap allocated object that has been deleted"
//@ diag: 21:15 note "in call to 'read()'"
//@ diag: 29:15 error "static assertion expression is not an integral constant expression"
//@ diag: 26:8 note "assignment to heap allocated object that has been deleted"
//@ diag: 29:15 note "in call to 'write()'"
//@ diag: 40:15 error "static assertion expression is not an integral constant expression"
//@ diag: 36:16 note "read of heap allocated object that has been deleted"
//@ diag: 40:15 note "in call to 'reuse()'"
constexpr int read() {
  int *p = new int(1);
  delete p;
  return *p;
}
static_assert(read() == 1);

constexpr int write() {
  int *p = new int[2]{1, 2};
  delete[] p;
  p[0] = 3;
  return 1;
}
static_assert(write());

// A freed block is reused, but the old pointer stays dangling.
constexpr int reuse() {
  int *p = new int(1);
  delete p;
  int *q = new int(2);
  int r = *q + *p;
  delete q;
  return r;
}
static_assert(reuse() == 3);
