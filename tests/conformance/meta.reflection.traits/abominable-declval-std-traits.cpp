// [meta.unary.prop], [meta.rel], [func.require]: the library traits built
// on `declval<U>()` (directly or through the builtins of
// abominable-declval-traits.cpp) are false for an abominable function
// type U: assignable, constructible, convertible, invocable, swappable.
// WP-1613.
//@ skip(clang): clang 22 answers true for is_assignable / is_constructible / is_convertible of an abominable function type
//@ skip(gcc): GCC 16.1 answers true for is_constructible of an abominable function type
//@ expect: compile-ok
#include <type_traits>
struct All {
  All();
  template <class T> All(T &&);
  template <class T> All &operator=(T &&);
};
using F = void() const;
using G = void() &&;
static_assert(!std::is_assignable_v<All &, F>);
static_assert(!std::is_nothrow_assignable_v<All &, G>);
static_assert(!std::is_constructible_v<All, F>);
static_assert(!std::is_convertible_v<G, All>);
static_assert(!std::is_nothrow_convertible_v<F, All>);
static_assert(!std::is_invocable_v<F>);
static_assert(!std::is_invocable_r_v<void, G>);
static_assert(!std::is_swappable_v<F>);
static_assert(!std::is_swappable_with_v<F, G>);
static_assert(!std::is_nothrow_swappable_v<G>);
static_assert(std::is_assignable_v<All &, void()>);
static_assert(std::is_invocable_v<void()>);
