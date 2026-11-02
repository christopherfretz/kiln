// [temp.deduct.call]/4.3, [temp.alias]/2: P = S<T> is deduced against the
// class type of a base of A; a base written through an alias template
// (`A<T>`, `P<T>` naming `S<T>`, `S<T*>`) is that class, so the sugar does
// not hide it. libstdc++ <variant>: `_Variant_base` is reached through
// `__detail::__variant::_Variant_base` aliases (variant:730, :1552).
// Verified against clang 19.
//@ expect: run
//@ strict-diags
//@ stdout: 1 2 3 4 5
#include <stdio.h>
template<class T> struct S { int v; };
template<class T> using A = S<T>;
template<class T> using P = S<T *>;
template<class T> struct D : A<T> {};
template<class T> struct E : P<T> {};
struct F : A<long> {};
template<class T> struct G : D<T> {};

template<class T> int put(S<T> &s) { return s.v; }
template<class T> int ptr(S<T *> &s) { return s.v; }
template<template<class> class TT, class U> int tt(TT<U> &s) { return s.v; }

int main() {
  D<int> d;
  d.v = 1;
  E<char> e;
  e.v = 2;
  F f;
  f.v = 3;
  G<short> g;
  g.v = 4;
  D<unsigned> h;
  h.v = 5;
  printf("%d %d %d %d %d\n", put(d), ptr(e), put(f), put(g), tt(h));
}
