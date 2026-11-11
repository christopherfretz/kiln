// GNU local labels (an extension; [stmt.label]): `__label__ a, b;` at
// the start of a block declares labels visible only in that block, which
// shadow the function's labels of the same name. A local label that is
// never defined is "use of undeclared label" at its declaration (even
// unused); outside its block the name is the function's label again.
// Defining one twice is a redefinition. `__label__` must name
// identifiers and end with `;`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:18 error "use of undeclared label 'b'"
//@ diag: 32:3 error "redefinition of label 'c'"
//@ diag: 30:15 note "previous definition is here"
//@ diag: 35:15 error "expected identifier"
//@ diag: 35:14 error "expected ';' at end of declaration"
//@ diag: 38:16 error "expected ';' at end of declaration"
//@ diag: 42:15 error "use of undeclared label 'e'"
//@ diag: 28:8 error "use of undeclared label 'a'"
//@ diag: 35:15 warning "expression result unused"
//@ diag: 53:10 error "use of undeclared label 'top'"
//@ diag: 59:15 error "use of undeclared label 'in_lambda'"
//@ diag: 57:13 error "use of undeclared label 'unused'"
void f(int x) {
  {
    __label__ a, b;
    if (x) goto a;
  a:;
  }
  goto a;
  {
    __label__ c;
  c:;
  c:;
  }
  {
    __label__ 1;
  }
  {
    __label__ d
  d:;
  }
  {
    __label__ e;
    {
      __label__ e;
    e:;
    }
  }
}
void g() {
  __label__ top;
top:
  [] {
    goto top;
  }();
}
void h() {
  __label__ unused;
  auto l = [] {
    __label__ in_lambda;
  };
}
