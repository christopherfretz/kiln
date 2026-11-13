// [temp.deduct.type]/20 (second sentence), [temp.spec.partial.match]/2:
// when the type of a non-type parameter `i` of a partial specialization
// differs from the type of the corresponding parameter of the template
// named by the enclosing simple-template-id (cv-qualifiers ignored),
// deduction fails, so the partial specialization does not match: `short
// s` against `template<int> A`, `unsigned N` against `template<unsigned
// long> Arr`, `int s` or `int... Is` against an `auto` parameter deduced
// as `char`. A parameter of the same type, a `const int` one, or one whose
// type is itself deduced (`template<class T, T t>`) matches. Class and
// variable templates alike. GCC 16.1 agrees; clang 19 and 22 match the
// first two (they check the rule for function templates only). WP-1405.
//@ expect: compile-ok
//@ std: c++20
//@ oracle: gcc
//@ skip(clang): clang 19/22 match a partial specialization whose non-type parameter type differs from the primary's
template <int i> struct A {};
template <const int i> struct CA {};
template <auto> struct V {};
template <class> struct X { static constexpr int v = 0; };
template <short s> struct X<A<s>> { static constexpr int v = 1; };
template <int s> struct X<CA<s>> { static constexpr int v = 2; };
template <int s> struct X<V<s>> { static constexpr int v = 3; };
template <class T, T t> struct X<V<t> *> { static constexpr int v = 4; };
static_assert(X<A<1>>::v == 0);
static_assert(X<CA<1>>::v == 2);
static_assert(X<V<1>>::v == 3);
static_assert(X<V<'c'>>::v == 0);
static_assert(X<V<1L> *>::v == 4);

template <unsigned long N> struct Arr {};
template <class> struct Z { static constexpr int v = 0; };
template <unsigned N> struct Z<Arr<N>> { static constexpr int v = 1; };
template <unsigned long N> struct Z<Arr<N> *> { static constexpr int v = 2; };
static_assert(Z<Arr<2>>::v == 0);
static_assert(Z<Arr<2> *>::v == 2);

template <auto... Vs> struct VL {};
template <class> struct W { static constexpr int v = 0; };
template <int... Is> struct W<VL<Is...>> { static constexpr int v = 1; };
static_assert(W<VL<1, 2>>::v == 1);
static_assert(W<VL<1, 'c'>>::v == 0);

template <class T> constexpr int vt = 0;
template <short s> constexpr int vt<A<s>> = 1;
template <int s> constexpr int vt<CA<s>> = 2;
static_assert(vt<A<1>> == 0);
static_assert(vt<CA<1>> == 2);
