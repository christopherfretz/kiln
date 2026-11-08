//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
// [dcl.constexpr]/6 is no diagnostic required: from C++23 clang reports a
// constexpr function that can never be constant only under
// -Winvalid-constexpr (off by default); before C++23 it is an error that
// -Wno-error=invalid-constexpr downgrades to a warning. WP-873.
int g();
int glob;
constexpr int f1() { return g(); }
constexpr int f2(int x) { return glob + x; }
struct S {
  constexpr S() { g(); }
  constexpr int mf() const { return g(); }
};
auto l1 = [](int x) constexpr { return g() + x; };
