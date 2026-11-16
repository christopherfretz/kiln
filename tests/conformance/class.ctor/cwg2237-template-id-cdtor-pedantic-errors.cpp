// CWG2237: under -pedantic-errors the C++20 template-id constructor and
// destructor declarators are errors, as in GCC 16.1 (spec/diag.md D18);
// clang 22 accepts them. Found running GCC's C++ testsuite (WP-1466).
//@ expect: compile-fail
//@ std: c++20
//@ args: -pedantic-errors
//@ strict-diags
//@ diag: 12:3 error "template-id not allowed for constructor in C++20"
//@ diag: 13:3 error "template-id not allowed for destructor in C++20"
//@ xfail(clang): clang 22 does not diagnose CWG2237 (spec/diag.md D18)
template<class T> struct X {
  X<T>();
  ~X<T>();
};
