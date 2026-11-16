// [dcl.stc]/2, [diff.cpp14.dcl.dcl]: `register` is deprecated in C++11 and
// C++14 (clang -Wdeprecated-register, on by default) and gone in C++17
// (clang -Wregister, an error by default). C++98 says nothing.
// Found running GCC's C++ testsuite (WP-1467).
//@ std: c++14
//@ strict-diags
//@ diag: 9:7 warning "'register' storage class specifier is deprecated and incompatible with C++17"
//@ diag: 10:3 warning "'register' storage class specifier is deprecated and incompatible with C++17"
int f(register int a) {
  register int x = a;
  return x;
}
