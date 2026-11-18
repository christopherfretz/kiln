// __integer_pack(N) as a part of a pack expansion's pattern, also beside
// another pack of the same length: each element substitutes the index.
// GCC 16.1 accepts only __integer_pack(N) as the whole pattern ("sorry,
// unimplemented"), which its own testsuite marks as a GCC bug (an xfail
// dg-bogus), so kiln expands any pattern. Found running GCC's C++
// testsuite. WP-1667.
//@ std: c++20
//@ expect: compile-ok
//@ skip(clang): clang 19/22 have no __integer_pack
//@ strict-diags
template <class T, class U> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;

template <int... I> struct A {};
template <int N> using twice = A<(2 * __integer_pack(N))...>;
static_assert(same<twice<3>, A<0, 2, 4>>);

template <int N, int... M> using plus = A<(__integer_pack(N) + M)...>;
static_assert(same<plus<3, 10, 20, 30>, A<10, 21, 32>>);

template <int N> constexpr int squares() { return (0 + ... + (__integer_pack(N) * __integer_pack(N))); }
static_assert(squares<4>() == 14);
