// [dcl.init.list]/3.10: a non-const lvalue reference cannot bind the
// temporary an initializer list creates; a single reference-related
// element binds directly.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:31 error "non-const lvalue reference to type 'std::initializer_list<int>' cannot bind to an initializer list temporary"
//@ diag: 14:46 error "volatile lvalue reference to type 'const volatile std::initializer_list<int>' cannot bind to an initializer list temporary"
//@ diag: 15:8 error "non-const lvalue reference to type 'int' cannot bind to an initializer list temporary"
//@ diag: 20:9 error "non-const lvalue reference to type 'long' cannot bind to an initializer list temporary"
//@ diag: 21:9 error "rvalue reference to type 'int' cannot bind to lvalue of type 'int'"
#include "Inputs/initializer_list.h"
void f() {
  std::initializer_list<int>& r = {1, 2};
  volatile const std::initializer_list<int>& v = {3};
  int& i = {4};
}
void g() {
  int x = 0;
  int& ok = {x};
  long& l = {x};
  int&& rr = {x};
  const int& c = {1L};
  (void)ok; (void)l; (void)rr; (void)c;
}
