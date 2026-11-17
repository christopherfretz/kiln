// [basic.def.odr]/10: a local variable is odr-usable only where every
// intervening scope is a block scope or the parameter scope of a lambda that
// can capture it; a local class's or enumeration's scope is neither, so
// odr-using a by-copy capture (or, outside any lambda, the variable) in an
// enumerator's value, a bit-field width, a member's array bound or a member's
// noexcept-specifier is ill-formed, inside a lambda body or not. clang 19 and
// 22 report each; GCC 16.1 rejects the enumerator and the bit-field inside a
// lambda body (a lambda capture is not a constant expression) and accepts the
// others: the text and clang decide. Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 31:20 error "reference to local variable 'i' declared in enclosing function 'f'"
//@ diag: 29:13 note "'i' declared here"
//@ diag: 33:17 error "reference to local variable 'i' declared in enclosing function 'f'"
//@ diag: 29:13 note "'i' declared here"
//@ diag: 34:17 error "reference to local variable 'i' declared in enclosing function 'f'"
//@ diag: 29:13 note "'i' declared here"
//@ diag: 35:27 error "reference to local variable 'i' declared in enclosing function 'f'"
//@ diag: 29:13 note "'i' declared here"
//@ diag: 37:25 error "reference to local variable 'i' declared in enclosing function 'f'"
//@ diag: 29:13 note "'i' declared here"
//@ diag: 37:5 warning "expression result unused"
//@ diag: 30:3 warning "expression result unused"
//@ diag: 42:18 error "reference to local variable 'n' declared in enclosing function 'h'"
//@ diag: 41:13 note "'n' declared here"
//@ diag: 43:24 error "reference to local variable 'n' declared in enclosing function 'h'"
//@ diag: 41:13 note "'n' declared here"
void f() {
  const int i = 2;
  [=] {
    enum E { a = *&i };
    struct L {
      int b : *&i;
      int arr[*&i];
      void g() noexcept(*&i == 2);
    };
    [] { enum F { c = *&i }; };
  };
}
void h() {
  const int n = 3;
  enum G { d = *&n };
  struct M { int w : *&n; };
}
