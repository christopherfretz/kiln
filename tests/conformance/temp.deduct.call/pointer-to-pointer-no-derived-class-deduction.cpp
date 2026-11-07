// [temp.deduct.call]/4.3: the derived-class rule applies when P is a
// simple-template-id or a pointer to one. Through a second pointer,
// `F<T0>**` against `X<A, B>**` is a plain type match: a mismatch fails
// deduction without looking at the bases of `X<A, B>`, so the class is not
// instantiated (clang's `isSimpleTemplateIdType` test). Boost.Proto
// `template_arity<F>` probes `mpl::and_<proto::_state, ...>` this way, and
// instantiating it is ill-formed (`nested_type_wknd<_state>`). The calls
// are qualified, so no argument-dependent lookup completes the class.
// Verified against clang 19.
//@ expect: run
//@ strict-diags
//@ stdout: 1 2 4 2
#include <stdio.h>

template<int N> struct I {};
template<class T> struct Bad { typedef typename T::type type; };
template<class A, class B, class C> struct And : Bad<A>::type {};
struct S {};

namespace d {
char h(...);
template<template<class> class F, class T0>
short h(F<T0> **, I<1> *);
template<template<class, class, class> class F, class T0, class T1, class T2>
int h(F<T0, T1, T2> **, I<3> *);
}

typedef And<S, S, S> X;
static_assert(sizeof(d::h((X **)0, (I<1> *)0)) == 1);
static_assert(sizeof(d::h((X **)0, (I<3> *)0)) == sizeof(int));

int main() {
  printf("%d %d %d %d\n", (int)sizeof(d::h((X **)0, (I<1> *)0)),
         (int)sizeof(d::h((X **)0, (I<2> *)0)) + 1,
         (int)sizeof(d::h((X **)0, (I<3> *)0)),
         (int)sizeof(d::h((X ***)0, (I<1> *)0)) + 1);
}
