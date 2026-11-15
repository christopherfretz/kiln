// [dcl.contract.func], [cpp.predefined] (P2900R14) with GCC 16.1's
// -fcontracts: function contract specifiers before C++26, on functions,
// members, function templates and lambdas, and __cpp_contracts 202502L.
// `contract_assert` stays a C++26 keyword (an ordinary name here, as in
// GCC). Found running GCC's C++ testsuite (g++.dg/contracts).
//@ expect: compile-ok
//@ std: c++23
//@ args: -fcontracts
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
static_assert(__cpp_contracts == 202502L);
int f(const int x) pre(x > 0) post(r: r > x) { return x + 1; }
struct S {
  int m(const int x) const pre(x > 0) post(r: r == x) { return x; }
};
template <class T> T t(const T x) pre(x > 0) post(r: r == x) { return x; }
auto l = [](int x) pre(x > 0) { return x; };
int contract_assert = 0;
int use() { return f(1) + S{}.m(1) + t(2) + l(1) + contract_assert; }
