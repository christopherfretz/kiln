// [temp.spec.partial.match]/2, [temp.deduct.type]/17, [temp.arg.nontype]/1:
// a value deduced for a partial specialization's non-type parameter is
// converted to that parameter's type as a converted constant expression,
// so a value the type cannot represent (signed or unsigned, a `bool`, a
// parameter after a type parameter) does not match. WP-958.
//@ std: c++20
//@ expect: compile-ok
template <long N> struct L { static constexpr int v = 0; };
template <signed char C> struct L<C> { static constexpr int v = 1; };
static_assert(L<127>::v == 1);
static_assert(L<128>::v == 0);
static_assert(L<-128>::v == 1);
static_assert(L<-129>::v == 0);
template <class T, int N> struct P { static constexpr int v = 0; };
template <class T, unsigned short N> struct P<T*, N> { static constexpr int v = 1; };
static_assert(P<int*, 65535>::v == 1);
static_assert(P<int*, 65536>::v == 0);
static_assert(P<int*, -1>::v == 0);
template <class T, T> struct V { static constexpr int v = 0; };
template <bool B> struct V<int, B> { static constexpr int v = 1; };
static_assert(V<int, 1>::v == 1);
static_assert(V<int, 2>::v == 0);
