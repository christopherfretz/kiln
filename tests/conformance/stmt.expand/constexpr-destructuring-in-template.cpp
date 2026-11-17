// [stmt.expand]/5.3, [dcl.struct.bind]/4, [expr.const] (P2686R5): a
// `constexpr` destructuring expansion statement over a prvalue declares
// `constexpr auto&& [u...]`. Where `get<i>` returns a prvalue, U_i is T_i:
// the holding variable is a `constexpr` object, usable in constant
// expressions, whatever T_i's constness. So the elements are constants, in
// a function template, a generic lambda, a class template's member, a
// nested expansion, and with zero elements, as they are outside a
// template. Found running GCC's C++ testsuite.
//@ args: --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang 22 does not implement expansion statements
//@ skip(gcc): GCC 16.1 has no P2686
//@ expect: compile-ok
#include <utility>
template <int N> constexpr int g() {
  int s = 0;
  template for (constexpr auto i : std::make_index_sequence<N>{}) { s += i; }
  return s;
}
static_assert(g<3>() == 3);
static_assert(g<0>() == 0);

constexpr auto lam = [](auto n) {
  int s = 0;
  template for (constexpr auto i : std::make_index_sequence<decltype(n)::value>{}) {
    constexpr auto k = i * 2;
    s += k;
  }
  return s;
};
static_assert(lam(std::integral_constant<int, 4>{}) == 12);

template <class T> struct S {
  static constexpr int sum() {
    int s = 0;
    template for (constexpr T i : std::make_integer_sequence<T, 3>{}) {
      template for (constexpr T j : std::make_integer_sequence<T, i>{}) {
        s += j;
      }
    }
    return s;
  }
};
static_assert(S<int>::sum() == 1);
static_assert(S<long>::sum() == 1);

template <int N> constexpr int bind() {
  constexpr auto &&[a, b, c] = std::make_index_sequence<N>{};
  constexpr auto i = a + b + c;
  return i;
}
static_assert(bind<3>() == 3);

constexpr int plain() {
  constexpr auto &&[a, b, c] = std::make_index_sequence<3>{};
  constexpr auto i = c;
  int s = 0;
  template for (auto j : std::make_index_sequence<3>{}) { s += j; }
  return i + s;
}
static_assert(plain() == 5);
