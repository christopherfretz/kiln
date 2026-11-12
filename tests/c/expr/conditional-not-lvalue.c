// C17 6.5.15 footnote 110: a conditional expression is not an lvalue,
// whatever its operands (structure or `void` lvalues included), so `&`
// (6.5.3.2p1) and assignment (6.5.16p2) reject it, as clang does.
//@ expect: compile-fail
//@ strict-diags
//@ args: -Wno-void-ptr-dereference
//@ diag: 14:9 error "cannot take the address of an rvalue of type 'struct S'"
//@ diag: 15:17 error "expression is not assignable"
//@ diag: 16:9 error "cannot take the address of an rvalue of type 'void'"
//@ diag: 17:9 error "cannot take the address of an rvalue of type 'void'"
//@ diag: 18:12 error "cannot take the address of an rvalue of type 'int'"
struct S { int a; } s1, s2;
void f(int c, void *p) {
  (void)&(c ? s1 : s2);
  (c ? s1 : s2) = s1;
  (void)&(c ? *p : *p);
  (void)&(c ? *p : (void)0);
  int *q = &(c ? s1 : s2).a;
  (void)q;
}
