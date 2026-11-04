// [meta.reflection.substitute], [meta.reflection.traits] (P2996R13),
// [meta.unary.prop]: substitute(^^V, args) of a variable template
// instantiates V<args...>, whose constexpr initializer is evaluated there
// ([temp.inst]/8). That evaluation is manifestly constant-evaluated, not
// part of the substitution's SFINAE trial: a consteval function it calls
// may ask the traits that run a trial initialization (constructible,
// assignable, destructible), including of a class template specialization
// that is instantiated by the query, and extract reads the answer
// (spec/reflection.md §4.3, WP-538). Compared with GCC 16.1.
//@ expect: compile-ok
//@ oracle: gcc
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#if __has_include(<meta>)
#include <meta>
#else
#include "kiln_meta_traits.h"
#endif

struct Point { double x, y; };
struct NoCopy { NoCopy(); NoCopy(const NoCopy&) = delete; };
struct NoDefault { NoDefault(int); };
struct ThrowMove { ThrowMove(ThrowMove&&) noexcept(false); };
template <class T> struct Box { T t; };

// The shape of the issue: one door, one trait.
consteval unsigned char outer(std::meta::info t) {
  return std::meta::is_copy_constructible_type(t) ? 4 : 0;
}
template <std::meta::info C> inline constexpr unsigned char outer_v = outer(C);
static_assert(std::meta::extract<unsigned char>(
                  std::meta::substitute(^^outer_v, {std::meta::reflect_constant(^^Point)})) == 4);

// Each trial-initialization trait, as a bit.
consteval unsigned bits(std::meta::info t) {
  using namespace std::meta;
  return unsigned(is_default_constructible_type(t)) | unsigned(is_copy_constructible_type(t)) << 1 |
         unsigned(is_move_constructible_type(t)) << 2 |
         unsigned(is_nothrow_move_constructible_type(t)) << 3 |
         unsigned(is_destructible_type(t)) << 4 | unsigned(is_trivially_copy_constructible_type(t)) << 5 |
         unsigned(is_constructible_type(t, {^^int})) << 6;
}
template <std::meta::info C> inline constexpr unsigned bits_v = bits(C);
consteval unsigned door(std::meta::info t) {
  using namespace std::meta;
  return extract<unsigned>(substitute(^^bits_v, {reflect_constant(t)}));
}

static_assert(door(^^Point) == 0b1111111);  // Point(1) is aggregate paren-init
static_assert(door(^^int) == 0b1111111);
static_assert(door(^^NoCopy) == 0b0010001);
static_assert(door(^^NoDefault) == 0b1111110);
static_assert(door(^^ThrowMove) == 0b0010100);
static_assert(door(^^Box<int>) == 0b1111111);  // Box<int> is instantiated by the query
static_assert(door(^^Box<NoCopy>) == 0b0010001);
// The direct template-id agrees.
static_assert(bits_v<^^Point> == 0b1111111);

int main() {}
