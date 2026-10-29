// [expr.delete]/2: the operand of delete must point to the complete object a
// new-expression created, not to an element, a member or past its end.
// Mirrors crates/kiln_oven/tests/new_delete.rs `expr_delete_faults`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:15 error "static assertion expression is not an integral constant expression"
//@ diag: 22:3 note "delete of pointer to subobject '&{*new S[2]#0}[1]'"
//@ diag: 25:15 note "in call to 'element()'"
//@ diag: 33:15 error "static assertion expression is not an integral constant expression"
//@ diag: 30:3 note "delete of pointer to subobject '&{*new S#0}.b'"
//@ diag: 33:15 note "in call to 'member()'"
//@ diag: 41:15 error "static assertion expression is not an integral constant expression"
//@ diag: 38:3 note "delete of pointer '&{*new int[3]#0}[3]' that does not point to complete object"
//@ diag: 41:15 note "in call to 'past_end()'"
struct S {
  int a, b;
};

constexpr int element() {
  S *p = new S[2]{};
  S *q = p + 1;
  delete[] q;
  return 1;
}
static_assert(element());

constexpr int member() {
  S *p = new S{1, 2};
  int *q = &p->b;
  delete q;
  return 1;
}
static_assert(member());

constexpr int past_end() {
  int *p = new int[3]();
  int *e = p + 3;
  delete[] e;
  return 1;
}
static_assert(past_end());
