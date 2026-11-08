// [temp.spec.partial.order]/1, [temp.deduct.type]/8 with CWG2398 as clang
// deduces it: `F<T1>` (`template<class> class F`) is deduced from the
// other partial specialization's `protect<T>`, whose second argument is
// the default `0` (kept as an expression while `T` is dependent), so
// `lambda<protect<T>, Tag>` is more specialized than `lambda<F<T1>, Tag>`.
// Boost.MPL's `lambda<protect<arg<1>>>` (Boost.Graph `src/graphml.cpp`)
// was ambiguous (WP-812).
//@ expect: compile-ok
//@ strict-diags
template<int N> struct int_ {};
template<class T = int, int not_le_ = 0> struct protect : T {};
template<int N> struct arg {};
template<class T, class Tag, class Arity = int_<1>> struct lambda { static constexpr int v = 0; };
template<template<class P1> class F, class T1, class Tag>
struct lambda<F<T1>, Tag, int_<1>> { static constexpr int v = 1; };
template<class T, class Tag> struct lambda<protect<T>, Tag, int_<1>> { static constexpr int v = 2; };
template<class T> struct box {};
static_assert(lambda<protect<arg<1>>, void>::v == 2);
static_assert(lambda<box<arg<1>>, void>::v == 1);
static_assert(lambda<protect<arg<1>, 1>, void>::v == 0);
