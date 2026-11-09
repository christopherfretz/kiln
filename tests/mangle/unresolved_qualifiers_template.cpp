// Mangling differential: non-member unresolved names with template
// qualifiers (spec/abi.md §8.15, WP-1061; [temp.over.link]/5): every
// qualifier level after a template-id, `template` member templates, a
// template template parameter (`srT_IT0_E`), an operator name (`on`), an
// enumerator and an `abi_tag`ged class template (the tag is not written).
namespace N {
  template<class T> struct X {
    static int g(T);
    template<class U> struct Z { struct Y { static int g(T); template<class V> static int k(V); }; static int g(T); };
    struct Y { template<class V> static int k(V); };
    static int operator()(T);
    enum { e = 1 };
  };
  template<class T> struct [[gnu::abi_tag("tg")]] W { static int g(T); };
}
struct A { template<class U> struct Z { struct Y { static int g(A); }; }; };
template<template<class> class TP, class T> auto f1(T t) -> decltype(TP<T>::g(t)) { return 0; }
template<class T> auto f2(T t) -> decltype(N::X<T>::template Z<int>::Y::g(t)) { return 0; }
template<class T> auto f3(T t) -> decltype(T::template Z<int>::Y::g(t)) { return 0; }
template<class T> auto f4(T t) -> decltype(N::X<T>::Y::template k<int>(t)) { return 0; }
template<class T> auto f5(T t) -> decltype(N::W<T>::g(t)) { return 0; }
template<class T> auto f6(T t) -> decltype(N::X<T>::e + t) { return 0; }
template<class T> auto f7(T t) -> decltype(N::X<T>::template Z<T>::Y::template k<T>(t)) { return 0; }
template<class T> auto f8(T t) -> decltype(::N::X<T>::template Z<int>::Y::g(t)) { return 0; }
template<class T> auto f9(T t) -> decltype(N::X<T>::operator()(t)) { return 0; }
template<class T, class U> auto f10(T t, U u) -> decltype(N::X<T>::template Z<U>::g(t)) { return 0; }
void use() {
  f1<N::X>(1); f2(1); f3(A()); f4(1); f5(1); f6(1); f7(1); f8(1); f9(1); f10(1, 2L);
}
