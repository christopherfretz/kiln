// [temp.res], [dcl.array]/6: cv-qualifiers applied to an array alias
// that names a template parameter ("QualifiedArray" sugar) apply to the
// substituted element type; substitution rebuilds the array and then
// re-applies the qualifiers.
//@ expect: run
//@ stdout: 2 6 3
#include <cstdio>

template <class T, int N> struct Z {
  using A = T[3];
  using B = T[];
  using C = T[N];
  const A a = {1, 2, 3};
  static T f(const A& x) { return x[1]; }
  using CA = const A;
  using VA = volatile A;
  using CB = const B;
  using CVC = const volatile C;
  static T sum(const C& c) {
    T s = 0;
    for (int i = 0; i < N; ++i) s += c[i];
    return s;
  }
};

static_assert(__is_same(Z<int, 1>::CA, const int[3]));
static_assert(__is_same(Z<long, 1>::VA, volatile long[3]));
static_assert(__is_same(Z<char, 1>::CB, const char[]));
static_assert(__is_same(Z<short, 4>::CVC, const volatile short[4]));

template <class T> using Arr2 = T[2];
template <class T> int g(const Arr2<T>& p) { return sizeof(p) / sizeof(T); }

int main() {
  int c[3] = {1, 2, 3};
  int x[2] = {0, 0};
  std::printf("%d %d %d\n", Z<int, 3>::f(Z<int, 3>().a), Z<int, 3>::sum(c),
              g<int>(x) + g(x) - 1);
}
