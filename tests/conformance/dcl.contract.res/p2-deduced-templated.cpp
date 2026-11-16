// [dcl.contract.res]/2 restricts a result name on a deduced return type
// to the definition only for a non-templated function: a function
// template (including an abbreviated one) or a member of a class
// template may declare it first and define it later, and the result
// name's type is known once the definition is instantiated. Found
// running GCC's C++ testsuite; GCC 16.1 accepts these.
// -fcontracts links libstdc++'s handler, as g++ needs.
//@ expect: run
//@ args: -fcontracts
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ stdout: 3 4 5
#include <cstdio>

auto g(auto &x) post(r: r >= 0);
template <class T> auto f() post(r: r > 0);
template <class T> struct S {
  auto h() post(r: r == 5);
};

auto g(auto &x) post(r: r >= 0) { return x; }
template <class T> auto f() post(r: r > 0) { return T(4); }
template <class T> auto S<T>::h() post(r: r == 5) { return T(5); }

int main() {
  int i = 3;
  std::printf("%d %d %d\n", g(i), f<int>(), S<int>().h());
}
