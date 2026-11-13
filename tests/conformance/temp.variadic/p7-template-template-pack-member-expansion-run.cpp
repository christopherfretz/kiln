// [temp.variadic]/5, /7, [temp.deduct.type]/9, [temp.arg.explicit]/9: in a
// member template of `S<template<class> class... TTs>`, the pattern
// `TTs<Us>` names the class's template template parameter pack and the
// member's own pack. Instantiating `S<A, B>` fixes `TTs` to {A, B} and
// leaves the expansion partially substituted; a call then expands both
// packs in lockstep, deducing `Us` element by element (or taking it from
// explicit arguments), in a parameter type, a return type, a member class
// template and a requires-clause, and as a template template argument
// (`W<TTs, Us>...`). clang 19 and 22 accept this in sema but crash when
// mangling the member (`Mangling declaration 'S<A, B>::g'`), so there is
// no clang oracle; GCC 16.1 rejects the deduction ("mismatched types 'Us'
// and 'int'"), contrary to [temp.deduct.type]/9.
//@ expect: run
//@ std: c++20
//@ skip(clang): clang 19 and 22 crash mangling a member whose type names a substituted template template parameter pack
//@ stdout: 35 35 71 -1 0
//@ stdout: 45
//@ stdout: 2 -1
//@ stdout: 16
//@ stdout: 1
extern "C" int printf(const char *, ...);

template <class... Ts> struct Z {};
template <class T> struct A { static constexpr int id = 1; };
template <class T> struct B { static constexpr int id = 2; };
template <class T> struct C { static constexpr int id = 3; };
template <template <class> class T, class U> struct W {
  static constexpr int v = T<U>::id * 10 + int(sizeof(U));
};
template <class T, class U> struct same { static constexpr bool value = false; };
template <class T> struct same<T, T> { static constexpr bool value = true; };

template <template <class> class... TTs> struct S {
  // Parameter type: deduced, or explicit.
  template <class... Us> static int g(Z<TTs<Us>...>) {
    return ((TTs<Us>::id * 10 + int(sizeof(Us))) + ... + 0);
  }
  static int g(...) { return -1; }
  // Return type, explicit arguments only.
  template <class... Us> static Z<TTs<Us>...> make() { return {}; }
  // Member class template.
  template <class... Us> struct In { using type = Z<TTs<Us>...>; };
  // Requires-clause.
  template <class... Us>
    requires(sizeof...(Us) == sizeof...(TTs)) && (same<TTs<Us>, TTs<Us>>::value && ...)
  static int req(Us...) { return 2; }
  static int req(...) { return -1; }
  // Template template argument.
  template <class... Us> static int h(Z<W<TTs, Us>...>) { return (W<TTs, Us>::v + ... + 0); }
};

static_assert(same<S<A, B>::In<int, char>::type, Z<A<int>, B<char>>>::value);
static_assert(same<decltype(S<C, A>::make<int, char>()), Z<C<int>, A<char>>>::value);

int main() {
  printf("%d %d %d %d %d\n", S<A, B>::g(Z<A<int>, B<char>>{}), S<B, A>::g(Z<B<char>, A<int>>{}),
         S<A, B, C>::g<long>(Z<A<long>, B<char>, C<short>>{}), S<A, B>::g(Z<A<int>>{}),
         S<>::g(Z<>{}));
  printf("%d\n", S<C, A>::g(S<C, A>::make<int, char>()));
  printf("%d %d\n", S<A, B>::req(1, 'c'), S<A, B>::req(1));
  printf("%d\n", S<A, A>::h(Z<W<A, int>, W<A, char>>{}) - 9);
  printf("%d\n", S<A>::g<int>(Z<A<int>>{}) == 14);
}
