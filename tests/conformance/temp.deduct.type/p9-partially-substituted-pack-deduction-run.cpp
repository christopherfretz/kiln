// [temp.deduct.type]/9: a template argument list whose pack expansions
// were partly substituted (a member function template of a class
// template specialization: `tup<Ts..., Us...>` with `Ts` known,
// `tup<pr<Ts, Us>...>`, `tup<Ps<Ts>...>`) deduces the remaining pack
// element by element; partial ordering ([temp.deduct.partial]/8)
// deduces one pack expansion from another.
//@ expect: run
//@ std: c++26
//@ stdout: 21 102 202 1 2 2
#include <cstdio>
template<class... Ts> struct tup {};
template<class A, class B> struct pr {};
template<class... Ts> struct O {
  template<class... Us> static int f(tup<Ts..., Us...>) { return 10 * sizeof...(Ts) + sizeof...(Us); }
  template<class... Us> static int g(tup<pr<Ts, Us>...>) { return 100 + sizeof...(Us); }
  template<template<class...> class... Ps> static int h(tup<Ps<Ts>...>) { return 200 + sizeof...(Ps); }
};
template<class... Ts> int po(tup<Ts...>) { return 1; }
template<class T, class... Ts> int po(tup<T, Ts...>) { return 2; }
template<class... Ts> int po2(void (*)(Ts...)) { return 1; }
template<class T, class... Ts> int po2(void (*)(T, Ts...)) { return 2; }
template<class T> struct W {};
template<class T> struct V {};
void fn(int, char) {}
int main() {
  std::printf("%d %d %d %d %d %d\n",
    O<int, char>::f(tup<int, char, long>{}),
    O<int, char>::g(tup<pr<int, long>, pr<char, short>>{}),
    O<int, char>::h(tup<W<int>, V<char>>{}),
    po(tup<>{}), po(tup<int>{}), po2(fn));
}
