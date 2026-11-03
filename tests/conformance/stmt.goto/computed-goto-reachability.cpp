// [stmt.goto] (GNU computed goto), [stmt.return]/4: an indirect `goto`
// has an edge to every label whose address the function takes (clang's
// indirect-goto block), so those labels are reachable for
// -Wunreachable-code and the fall-off check behind -Wreturn-type; other
// code after it is not, and an address taken only in an unevaluated
// operand gives no edge.
//@ expect: compile-fail
//@ strict-diags
//@ args: -Wunreachable-code-aggressive -Wreturn-type -Werror=return-type
//@ diag: 30:1 error "non-void function does not return a value in all control paths"
//@ diag: 34:7 warning "code will never be executed"
//@ diag: 42:3 warning "code will never be executed"
int table(int i) {
  static void *t[] = {&&a, &&b};
  goto *t[i];
a:
  i += 1;
  return i;
b:
  i += 2;
  return i;
}
int pick(int i) {
  void *p = i ? &&x : &&y;
  goto *p;
x:
  return 1;
y:
  ;
}
int skipped(int i) {
  void *p = &&a;
  goto *p;
  i = 3;
a:
  return i;
}
int unevaluated(int i) {
  void *p = sizeof(&&a) ? nullptr : nullptr;
  goto *p;
a:
  i += 1;
}
