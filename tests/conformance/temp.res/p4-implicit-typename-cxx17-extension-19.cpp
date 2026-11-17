// clang 19's wording of the implicit `typename` extension warning
// (p4-implicit-typename-cxx17-extension.cpp).
// Found running GCC's C++ testsuite.
//@ expect: compile-ok
//@ args: -std=c++17
//@ requires: clang<22
//@ strict-diags
//@ diag: 12:11 warning "missing 'typename' prior to dependent type name Outer<T *>::Value; implicit 'typename' is a C++20 extension [-Wc++20-extensions]"

template <class T> struct Outer {
  typedef int Value;
  typedef Outer<T *>::Value V1;
};
