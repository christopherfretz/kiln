// [dcl.fct.def.default]/1: only a special member function may be
// explicitly defaulted before C++20; clang leaves comparison operators out
// of its "only special member functions may be defaulted" text there.
//@ std: c++17
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:14 error "only special member functions may be defaulted"
//@ diag: 12:16 error "only special member functions may be defaulted"
//@ diag: 16:9 error "only special member functions may be defaulted"
struct S {
  void f() = default;
  int g(int) = default;
  S() = default;
  S &operator=(const S &) = default;
};
int x = default;
