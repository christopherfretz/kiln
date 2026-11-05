// [temp.names]/4 (C++11, N1757): `>>` closes two template argument lists;
// a parenthesized shift is still a shift. WP-646 feature probe.
//@ expect: compile-ok
//@ std: c++11
//@ strict-diags
template <class T> struct V { typedef T type; };
template <int N> struct I { static const int value = N; };
V<V<int>> a;
V<V<V<int>>> b;
static_assert(I<(8 >> 1)>::value == 4, "parenthesized shift");
static_assert(I<V<I<2>>::type::value>::value == 2, "nested");
typedef V<I<1>> VI;
