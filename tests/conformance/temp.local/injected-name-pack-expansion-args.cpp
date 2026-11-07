// [temp.local]/1, [temp.variadic]/5: inside a class template, its name
// with a pack expansion for a parameter that is not a pack (`Ts<T...>`
// in `template<template<class> class H, template<class> class... T>
// struct Ts`) names a dependent specialization, not the current
// instantiation, and is checked once substituted (googletest's
// `Templates<Head_, Tail_...>::Tail = Templates<Tail_...>`).
//@ expect: run
//@ strict-diags
//@ std: c++17
//@ stdout: 1 2 3 2
#include <cstdio>
template <template <class> class Tmpl> struct Sel {
  template <class T> struct Bind { typedef Tmpl<T> type; };
};
template <template <class> class H, template <class> class... T>
struct Templates {
  using Head = Sel<H>;
  using Tail = Templates<T...>;
  static constexpr int size = 1 + Tail::size;
};
template <template <class> class H> struct Templates<H> {
  using Head = Sel<H>;
  static constexpr int size = 1;
};
template <class H, class... T> struct Types {
  using Tail = Types<T...>;
  static constexpr int size = 1 + Tail::size;
};
template <class H> struct Types<H> { static constexpr int size = 1; };
template <class T> struct A { static constexpr int v = 1; };
template <class T> struct B { static constexpr int v = 2; };
template <class T> struct C { static constexpr int v = 3; };
int main() {
  using L = Templates<A, B, C>;
  std::printf("%d %d %d %d\n", L::Head::Bind<int>::type::v, L::Tail::Head::Bind<int>::type::v,
              L::size, Types<int, char>::size);
}
