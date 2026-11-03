// [stmt.goto] (GNU labels as values), [expr.prim.lambda.closure],
// [class.local]: a label belongs to the function body that contains it, so
// `&&l` in a lambda body or a member function of a local class does not
// find a label of the enclosing function: "use of undeclared label".
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:30 error "use of undeclared label 'l'"
//@ diag: 13:28 error "use of undeclared label 'l'"
void f() {
l:
  auto la = [] { void *p = &&l; (void)p; };
  struct S {
    void m() { void *p = &&l; (void)p; }
  };
  (void)la;
}
