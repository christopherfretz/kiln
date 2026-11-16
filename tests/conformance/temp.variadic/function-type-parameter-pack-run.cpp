// [dcl.fct]/5, /16, [temp.variadic]/5: a function parameter pack declared
// with a function type (`T... x(int)`, or unnamed) expands that type, each
// element adjusted to a pointer to function when the pack is substituted
// (mangled `DpFT_iE`, as clang and GCC do).
//@ expect: run
//@ std: c++17
//@ stdout: 3 5
#include <cstdio>

template <class... T> int named(T... x(int)) { return (0 + ... + int(x(1))); }
template <class... T> int unnamed(T... (int)) { return sizeof...(T); }

int f1(int a) { return a; }
double f2(int a) { return a + 1.0; }

int main() {
  int a = named<int, double>(f1, f2);
  int b = unnamed<int, double>(f1, f2) + 3;
  std::printf("%d %d\n", a, b);
}
