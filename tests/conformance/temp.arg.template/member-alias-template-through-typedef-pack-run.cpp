// [temp.arg.template]/1, [temp.variadic]/5: `Q::fn`, with `Q` a typedef of
// the specialization `quote<long>`, names the member alias template `fn`,
// so `defer<Q::fn, T...>` in an alias template is a template-id with a
// template template argument followed by a pack expansion. Boost.Mp11
// `mp_defer<Q_add_pointer::fn, T...>` (mp_defer.cpp:38) hits this.
//@ expect: run
//@ stdout: 8
extern "C" int printf(const char*, ...);
template<template<class...> class F, class... T> struct defer { using type = F<T...>; };
template<class X> struct quote { template<class... T> using fn = X; };
using Q = quote<long>;
template<class... T> using impl = defer<Q::fn, T...>;

int main() { printf("%d\n", (int)sizeof(impl<char>::type)); }
