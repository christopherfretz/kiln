// [expr.delete]/2, [expr.const]/5.20: the operand of delete must point to a
// live heap allocation; deleting it twice is not a constant expression.
// Mirrors crates/kiln_oven/tests/new_delete.rs `expr_delete_faults`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:15 error "static assertion expression is not an integral constant expression"
//@ diag: 15:3 note "delete of pointer that has already been deleted"
//@ diag: 18:15 note "in call to 'twice()'"
//@ diag: 27:15 error "static assertion expression is not an integral constant expression"
//@ diag: 24:3 note "delete of pointer that has already been deleted"
//@ diag: 27:15 note "in call to 'twice_array(3)'"
constexpr int twice() {
  int *p = new int(1);
  delete p;
  delete p;
  return 1;
}
static_assert(twice());

constexpr int twice_array(int n) {
  int *p = new int[n]();
  int *q = p;
  delete[] p;
  delete[] q;
  return 1;
}
static_assert(twice_array(3));
