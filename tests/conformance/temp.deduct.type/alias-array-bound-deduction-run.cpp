// [temp.deduct.type]/8, /20, [temp.alias]/2: a parameter type written with
// an alias template whose aliased type is an array, `A<N>&` for
// `template <int N> using A = int[N]`, is the array type `int (&)[N]`, so
// the bound is deduced from the argument's array type (`N = 3` from
// `int[3]`), also with a type and a bound (`Arr<T, N>`), a `bool` bound
// (`int[B]`, `B = true` from `int[1]`) and an `unsigned char` one. The
// deduced specializations are called through declarations whose asm
// labels are clang's mangled names (the alias is transparent there), so a
// different kiln mangling fails to link (spec/sema-templates.md §4.10,
// WP-1168).
//@ expect: run
//@ stdout: 3 51 1 5
//@ stdout: 3 51 1 5
#include <cstdio>

template <int N> using A = int[N];
template <class T, int N> using Arr = T[N];
template <bool B> using BA = int[B];
template <int N> int f(A<N>&) { return N; }
template <class T, int N> int g(const Arr<T, N>&) { return N * 10 + int(sizeof(T)); }
template <bool B> int h(BA<B>&) { return B; }
template <unsigned char N> int u(A<N>&) { return N; }
int f3(int (&)[3]) __asm__("_Z1fILi3EEiRAT__i");
int g5(const char (&)[5]) __asm__("_Z1gIcLi5EEiRAT0__KT_");
int h1(int (&)[1]) __asm__("_Z1hILb1EEiRAT__i");
int u5(int (&)[5]) __asm__("_Z1uILh5EEiRAT__i");

int main() {
  int a[3] = {};
  char c[5] = {};
  int b[1] = {};
  int d[5] = {};
  std::printf("%d %d %d %d\n", f(a), g(c), h(b), u(d));
  std::printf("%d %d %d %d\n", f3(a), g5(c), h1(b), u5(d));
}
