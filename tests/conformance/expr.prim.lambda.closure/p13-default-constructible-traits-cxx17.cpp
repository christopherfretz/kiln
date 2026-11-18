// C++17 [expr.prim.lambda.closure]/11: a closure type has no default
// constructor and a deleted copy assignment operator, with or without a
// lambda-capture. Every way of asking agrees: the built-in traits, the
// library trait and SFINAE on `T()` are false for a captureless closure
// (clang 22, clang 19 and GCC 16.1 agree; clang 22 only adds a stray
// "no matching constructor" error under a FAILED `static_assert` of
// `__is_constructible`). `T t;` and `T()` are rejected in
// p13-special-members-by-std.cpp; from C++20 see
// p13-special-members-by-std-accepted.cpp (WP-1652).
//@ expect: compile-ok
//@ args: -std=c++17
#include <type_traits>
template <class T, class = decltype(T())> constexpr bool value_init(int) { return true; }
template <class T> constexpr bool value_init(...) { return false; }
auto ns = [] { return 1; };
using N = decltype(ns);
static_assert(!__is_constructible(N));
static_assert(!__is_trivially_constructible(N));
static_assert(!__is_nothrow_constructible(N));
static_assert(!std::is_default_constructible_v<N>);
static_assert(!value_init<N>(0));
static_assert(__is_constructible(N, const N &));
static_assert(std::is_move_constructible_v<N>);
static_assert(!std::is_copy_assignable_v<N>);
static_assert(!__is_trivial(N));
void f() {
  auto local = [] {};
  using L = decltype(local);
  static_assert(!__is_constructible(L));
  static_assert(!std::is_default_constructible_v<L>);
  static_assert(!value_init<L>(0));
  static_assert(std::is_trivially_copy_constructible_v<L>);
}
