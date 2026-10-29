// [expr.delete]/2: the operand of delete must be a pointer to an object
// created by a new-expression; deleting an automatic or static object is not a
// constant expression.
// Mirrors crates/kiln_oven/tests/new_delete.rs `expr_delete_faults`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:15 error "static assertion expression is not an integral constant expression"
//@ diag: 19:3 note "delete of pointer '&x' that does not point to a heap-allocated object"
//@ diag: 22:15 note "in call to 'local()'"
//@ diag: 18:7 note "declared here"
//@ diag: 28:15 error "static assertion expression is not an integral constant expression"
//@ diag: 25:3 note "delete of pointer '&global' that does not point to a heap-allocated object"
//@ diag: 28:15 note "in call to 'static_storage()'"
//@ diag: 15:5 note "declared here"
int global;

constexpr int local() {
  int x = 0;
  delete &x;
  return 1;
}
static_assert(local());

constexpr int static_storage() {
  delete &global;
  return 1;
}
static_assert(static_storage());
