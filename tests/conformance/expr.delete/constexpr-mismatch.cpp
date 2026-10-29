// [expr.delete]/2: delete of an array allocation, or delete[] of a non-array
// allocation, is undefined and therefore not a constant expression. The
// allocations are made in other functions so that clang's
// -Wmismatched-new-delete stays quiet.
// Mirrors crates/kiln_oven/tests/new_delete.rs `expr_delete_faults`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:15 error "static assertion expression is not an integral constant expression"
//@ diag: 21:3 note "non-array delete used to delete pointer to array object of type 'int[3]'"
//@ diag: 24:15 note "in call to 'scalar_delete_of_array()'"
//@ diag: 16:33 note "heap allocation performed here"
//@ diag: 31:15 error "static assertion expression is not an integral constant expression"
//@ diag: 28:3 note "array delete used to delete pointer to non-array object of type 'int'"
//@ diag: 31:15 note "in call to 'array_delete_of_scalar()'"
//@ diag: 17:34 note "heap allocation performed here"
constexpr int *array() { return new int[3](); }
constexpr int *scalar() { return new int(1); }

constexpr int scalar_delete_of_array() {
  int *p = array();
  delete p;
  return 1;
}
static_assert(scalar_delete_of_array());

constexpr int array_delete_of_scalar() {
  int *p = scalar();
  delete[] p;
  return 1;
}
static_assert(array_delete_of_scalar());
