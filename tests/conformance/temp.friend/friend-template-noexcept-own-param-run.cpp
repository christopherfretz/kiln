// [temp.friend]/1, [except.spec]/5, [temp.deduct.call]: a friend function
// template of a class template whose noexcept-specifier names its own
// parameter, defined at namespace scope with the same specifier: calls
// deduce it, and each specialization's specifier is its own argument's.
// Reduced from g++.dg cpp0x/noexcept40.C.
//@ expect: run
//@ oracle: gcc
//@ skip(clang): clang keeps the friend's own parameter at its pattern depth, so the equivalent specifiers mismatch
//@ stdout: 1 0 3
#include <cstdio>
template <int N> struct Test {
  int v = N + 1;
  template <int M> friend int test(Test<M> &arg) noexcept(M == 0);
};
template <int N> int test(Test<N> &arg) noexcept(N == 0) { return arg.v; }
int main() {
  Test<0> t0;
  Test<2> t2;
  static_assert(noexcept(test(t0)));
  static_assert(!noexcept(test(t2)));
  std::printf("%d %d %d\n", test(t0), int(noexcept(test(t2))), test(t2));
}
