// [temp.param]/15, [expr.prim.lambda.closure]/1: a lambda as the default
// argument of a template parameter of a static data member template of
// a nested class template, with a class NTTP `N` used inside it. Found running GCC's C++ testsuite (WP-1472).
//@ expect: compile-ok
//@ std: c++20
//@ oracle: gcc
//@ skip(clang): clang 19 rejects the defaulted lambda (clang 22 accepts)
//@ xfail(kiln): WP-1497, `N.n + y` in the defaulted generic lambda sees `N` as void ("member reference base type 'void'")
struct X { int n; };
template <class T> struct A {
  template <class U> struct B {
    template <auto N, auto F = [](auto y) { return decltype(N){N.n + y}; }>
    static inline auto v = F;
    template <class V> static void f() { static_assert(v<X{1}>(42).n == 43); }
  };
};
int main() { A<int>::B<int>::f<int>(); }
