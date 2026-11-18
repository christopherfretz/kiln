// [expr.alignof] with GNU `__attribute__((aligned))` on a typedef named as
// a template argument: a function or variable template is instantiated
// from its canonical arguments (clang instantiates with the canonical
// converted arguments; GCC drops the attribute with -Wignored-attributes),
// so in `f<D>()` the parameter `U` is `double`, aligned to 8, for
// `alignof(U)`, `alignof(const U)`, a local `U` and a member of a local
// class, whichever spelling the first use had. A typedef declared inside
// the template keeps its own `aligned`, and a class template's arguments
// were canonical already. Found running GCC's C++ testsuite (the sibling
// of `alignof` of a cv-qualified aligned array typedef).
//@ std: c++20
//@ expect: run
//@ stdout: 8 8 8 8 8 8 16 136 64 8 8 | 8 8 8 32 32
#include <cstdio>
typedef double D __attribute__((aligned(32)));
typedef double T[4][4] __attribute__((aligned(32)));
template <class U> int f1() { return alignof(U); }
template <class U> int f2() { return alignof(const volatile U); }
template <class U> int f3() { U x; return alignof(x); }
template <class U> constexpr int vt = alignof(U);
template <class U> int f4() {
  struct L { char c; U u; };
  return sizeof(L);
}
template <class U> int f5() {
  typedef U X __attribute__((aligned(64)));
  return alignof(X);
}
template <class U> struct S { static int g() { return alignof(U); } };
template <class U> int f6(U) { return alignof(U); }
int main() {
  auto l = []<class U>() { return (int)alignof(U); };
  D d = 0;
  // The first uses name the typedefs; the later ones the canonical types.
  std::printf("%d %d %d %d %d %d %d %d %d %d %d | ", f1<D>(), f2<D>(), f3<D>(), vt<D>,
              f1<T>(), l.operator()<D>(), f4<D>(), f4<T>(), f5<D>(), S<D>::g(), f6<D>(d));
  std::printf("%d %d %d %d %d\n", f1<double>(), vt<double>, f3<double>(), (int)alignof(D),
              (int)alignof(T));
}
