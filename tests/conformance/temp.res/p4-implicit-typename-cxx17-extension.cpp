// [temp.res.general]/4 (P0634R3): where a dependent qualified name can
// only be a type (a member declaration's decl-specifier, here), C++20
// implies `typename`; before C++20 clang accepts it as an extension, with
// a -Wc++20-extensions warning.
// Found running GCC's C++ testsuite.
//@ expect: compile-ok
//@ args: -std=c++17
//@ strict-diags
//@ requires: clang>=22
//@ diag: 16:11 warning "missing 'typename' prior to dependent type name 'Outer<T *>::Value' is a C++20 extension [-Wc++20-extensions]"
//@ diag: 17:11 warning "missing 'typename' prior to dependent type name 'Outer<T *>::Inner' is a C++20 extension [-Wc++20-extensions]"

template <class T> struct Outer {
  typedef int Value;
  template <class> struct Inner {};
  typedef Outer<T *>::Value V1;
  typedef Outer<T *>::template Inner<int> V2;
  typedef typename Outer<T *>::Value V3;
  typedef Outer::Value V4;
};
