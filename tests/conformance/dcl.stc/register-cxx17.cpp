// [dcl.stc]/2, [diff.cpp14.dcl.dcl]: `register` is not a storage class
// specifier in C++17 (clang: an error by default, -Wregister). Found running
// GCC's C++ testsuite (WP-1467).
//@ std: c++17
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:7 error "ISO C++17 does not allow 'register' storage class specifier"
//@ diag: 10:3 error "ISO C++17 does not allow 'register' storage class specifier"
int f(register int a) {
  register int x = a;
  return x;
}
