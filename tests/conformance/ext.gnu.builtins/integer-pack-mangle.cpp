// __integer_pack(N) in a function template's signature mangles as GCC
// mangles it, a call of the builtin by name (`cl 14__integer_pack <N> E`):
// the asm label below names the specialization of get by GCC's symbol.
// (`m`: size_t is unsigned long on kiln's LP64 targets.) Found running
// GCC's C++ testsuite. WP-1667.
//@ std: c++20
//@ expect: run
//@ oracle: gcc
//@ skip(clang): clang 19/22 have no __integer_pack
//@ stdout: 7 7
#include <cstdio>

typedef __SIZE_TYPE__ size_t;
template <typename T, T... I> struct integer_sequence {};
template <typename T, T N>
using make_integer_sequence = integer_sequence<T, __integer_pack(N)...>;
template <typename... Ts>
using index_sequence_for = make_integer_sequence<size_t, sizeof...(Ts)>;
template <typename...> struct tuple {};

template <typename... Ts>
int get(tuple<index_sequence_for<Ts...>, Ts...>) { return 7; }

int by_symbol(tuple<index_sequence_for<>>) __asm__(
    "_Z3getIJEEi5tupleIJ16integer_sequenceImJXspcl14__integer_packsZT_EEEEDpT_EE");

int main() {
  int a = get(tuple<index_sequence_for<>>{});
  int b = by_symbol(tuple<index_sequence_for<>>{});
  std::printf("%d %d\n", a, b);
}
