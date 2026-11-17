// [dcl.struct.bind]/4, [expr.const] (P2686R5): in a `constexpr`
// tuple-like structured binding whose `get<i>` returns a prvalue, U_i is
// T_i, so each holding variable is a `constexpr` object of non-const
// type T_i, usable in constant expressions: the bindings are constants,
// at block and namespace scope, in a template, and through a conversion
// from `get`'s type to T_i. Found running GCC's C++ testsuite.
//@ skip(clang): clang 22 does not implement P2686R5 constexpr structured bindings
//@ skip(gcc): GCC 16.1 does not implement P2686R5 constexpr references to automatic objects
//@ expect: compile-ok
namespace std {
template <class> struct tuple_size;
template <decltype(sizeof 0), class> struct tuple_element;
} // namespace std
struct Q {};
template <> struct std::tuple_size<Q> { static constexpr int value = 2; };
template <decltype(sizeof 0) I> struct std::tuple_element<I, Q> { using type = long; };
template <decltype(sizeof 0) I> constexpr int get(Q) { return int(I) + 7; }

constexpr auto &&[x, y] = Q{};
static_assert(x == 7 && y == 8);

constexpr long f() {
  constexpr auto &&[a, b] = Q{};
  constexpr long c = a * b;
  return c;
}
static_assert(f() == 56);

template <class T> constexpr long g() {
  constexpr auto &&[a, b] = T{};
  constexpr long c = b - a;
  return c;
}
static_assert(g<Q>() == 1);
