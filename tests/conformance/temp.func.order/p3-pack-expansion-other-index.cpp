//@ expect: run
//@ stdout: 2 2 2 2 2
// [temp.func.order]/3, [temp.deduct.type]/9: in partial ordering a pack
// expansion of the other template's own pack (`L<I...>`) is deduced as a
// whole by a pack expansion, whatever template-parameter index each pack
// has (folly Fingerprint.cpp copy_table).
#include <cstdio>
#include <utility>
#include <array>
template <class... T> struct L {};
template <int... V> struct VV {};
template <class... I> int a(L<I...>) { return 1; }
template <class... I> int a(L<int, I...>) { return 2; }
template <class T, class... I> int b(T, L<I...>) { return 1; }
template <class T, class... I> int b(T*, L<T, I...>) { return 2; }
template <int N, int... I> int c(VV<N, I...>) { return 2; }
template <int... I> int c(VV<I...>) { return 1; }
template <class... I> int d(L<I...>, L<I...>) { return 1; }
template <class X, class... I> int d(L<X, I...>, L<X, I...>) { return 2; }
template <typename D, std::size_t S0, std::size_t... I0>
constexpr auto copy_table(D const (&t)[S0], std::index_sequence<I0...>) { return std::array<D, S0>{{t[I0]...}}; }
template <typename D, std::size_t S0>
constexpr auto copy_table(D const (&t)[S0]) { return copy_table(t, std::make_index_sequence<S0>{}); }
template <typename D, std::size_t S0, std::size_t S1, std::size_t... I0>
constexpr auto copy_table(D const (&t)[S0][S1], std::index_sequence<I0...>) {
  return std::array<std::array<D, S1>, S0>{{copy_table(t[I0])...}};
}
template <typename D, std::size_t S0, std::size_t S1>
constexpr auto copy_table(D const (&t)[S0][S1]) { return copy_table(t, std::make_index_sequence<S0>{}); }
constexpr int tab[2][3] = {{1, 2, 3}, {4, 5, 6}};
constexpr auto ct = copy_table(tab);
static_assert(ct[1][2] == 6);
int main() {
  int x = 0;
  std::printf("%d %d %d %d %d\n", a(L<int, char>{}), b(&x, L<int, char>{}), c(VV<1, 2>{}),
              d(L<int>{}, L<int>{}), ct[0][1]);
}
