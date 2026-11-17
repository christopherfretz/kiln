// [basic.def.odr]/10: a local class's default member initializer is in its
// class scope, so odr-using an enclosing function's local variable (or a
// lambda's by-copy capture) there is ill-formed. GCC 16.1 rejects; clang 19
// and 22 accept, the text and GCC decide.
//@ expect: compile-fail
//@ strict-diags
//@ oracle: gcc
//@ xfail(clang): clang 19 and 22 accept an odr-use of an enclosing local in a local class's default member initializer
//@ diag: 16:26 error "reference to local variable 'i' declared in enclosing function 'f'"
//@ diag: 14:13 note "'i' declared here"
//@ diag: 21:24 error "reference to local variable 'n' declared in enclosing function 'g'"
//@ diag: 20:13 note "'n' declared here"
void f() {
  const int i = 2;
  (void)[=] {
    struct L { int m = *&i; };
  };
}
void g() {
  const int n = 3;
  struct M { int w = *&n; };
}
