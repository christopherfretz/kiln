// [temp.deduct.type]/20, [temp.spec.partial.match]/2, [temp.arg.nontype]/1:
// when a partial specialization's non-type parameter has a type other than
// that of the corresponding primary parameter (`signed char C` for `long
// N`, `unsigned short N` for `int N`), deduction fails, so the partial
// specialization matches no value, in range or not. Before WP-1405 kiln
// converted the value and matched when it fit (clang's behaviour: clang
// 19/22 do not check the type rule for class templates); GCC 16.1 is the
// oracle. The dependent-type case is narrowing-nttp-dependent-type-clang19.cpp.
// WP-958; strict matching WP-1405.
//@ oracle: gcc
//@ skip(clang): clang 19/22 match a partial specialization whose non-type parameter type differs from the primary's
//@ std: c++20
//@ expect: compile-ok
template <long N> struct L { static constexpr int v = 0; };
template <signed char C> struct L<C> { static constexpr int v = 1; };
static_assert(L<127>::v == 0);
static_assert(L<128>::v == 0);
static_assert(L<-128>::v == 0);
static_assert(L<-129>::v == 0);
template <class T, int N> struct P { static constexpr int v = 0; };
template <class T, unsigned short N> struct P<T*, N> { static constexpr int v = 1; };
static_assert(P<int*, 65535>::v == 0);
static_assert(P<int*, 65536>::v == 0);
static_assert(P<int*, -1>::v == 0);
