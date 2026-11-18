// [expr.cond]/5 holds for ordinary objects and bit-fields only (clang
// `isOrdinaryOrBitFieldObject`): a conditional whose operand is a GNU
// vector element (`c ? v[0] : w[1]`, the other operand a vector element
// or an `int` lvalue) is a prvalue, so it cannot be assigned, incremented,
// bound to a non-const reference or have its address taken. kiln made it
// an lvalue, then stored to the wrong element (WP-1700). GCC 16.1 accepts
// all of it as an lvalue; clang 22 decides. Reads: ...-prvalue-run.cpp.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:21 error "expression is not assignable"
//@ diag: 22:21 error "expression is not assignable"
//@ diag: 23:3 error "expression is not assignable"
//@ diag: 24:18 error "expression is not assignable"
//@ diag: 25:18 error "expression is not assignable"
//@ diag: 26:8 error "non-const lvalue reference to type 'int' cannot bind to a temporary of type 'int'"
//@ diag: 27:28 error "expression is not assignable"
//@ diag: 28:12 error "cannot take the address of an rvalue of type 'int'"
typedef int v4 __attribute__((vector_size(16)));
void g(int c, v4 v, v4 w, int s) {
  // Line 21 next.
  (c ? v[0] : w[1]) = 10;
  (c ? v[1] : v[2]) += 100;
  ++(c ? w[3] : v[3]);
  (c ? v[2] : s) = 50;
  (c ? s : w[0]) -= 1;
  int &r = c ? v[3] : w[2];
  int x = (c ? v[0] : w[1])++;
  int *p = &(c ? v[0] : w[1]);
}
