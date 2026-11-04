// [meta.reflection.substitute], [meta.reflection.traits] (P2996R13),
// [temp.inst]/8: memo doors. A variable template memoises a reflection
// query; a consteval function reaches it with
// extract<T>(substitute(^^v, {reflect_constant(c)})), so the
// specialization's initializer runs inside the substitution. Here the
// outer door's initializer opens an inner door on a second variable
// template, and both ask constructibility, assignability and
// destructibility traits: each nested initializer is a manifestly
// constant-evaluated evaluation, not a trial, so every trait answers
// (spec/reflection.md §4.3, WP-538). A second query through the same door
// reads the memoised specialization. Compared with GCC 16.1.
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
using namespace std::meta;

struct Point { double x, y; };
struct Pinned { Pinned(); Pinned(Pinned&&) = delete; };
struct Locked { Locked& operator=(const Locked&) = delete; };
template <class T> struct Pair { T a, b; };

// The inner door: one trait word per type.
consteval unsigned char inner(info t) {
  return (is_copy_constructible_type(t) ? 1 : 0) | (is_move_constructible_type(t) ? 2 : 0) |
         (is_copy_assignable_type(t) ? 4 : 0) | (is_destructible_type(t) ? 8 : 0);
}
template <info C> inline constexpr unsigned char inner_v = inner(C);
consteval unsigned char inner_door(info t) {
  return extract<unsigned char>(substitute(^^inner_v, {reflect_constant(t)}));
}

// The outer door: its own traits, then the inner door's word of the
// type and of Pair<type>.
consteval unsigned outer(info t) {
  unsigned own = (is_default_constructible_type(t) ? 1u : 0u) |
                 (is_nothrow_move_constructible_type(t) ? 2u : 0u);
  unsigned in = inner_door(t);
  unsigned pair = inner_door(substitute(^^Pair, {t}));
  return own | in << 4 | pair << 8;
}
template <info C> inline constexpr unsigned outer_v = outer(C);
consteval unsigned outer_door(info t) {
  return extract<unsigned>(substitute(^^outer_v, {reflect_constant(t)}));
}

static_assert(outer_door(^^Point) == (0x3 | 0xF << 4 | 0xF << 8));
static_assert(outer_door(^^int) == (0x3 | 0xF << 4 | 0xF << 8));
static_assert(outer_door(^^Pinned) == (0x1 | 0x8 << 4 | 0x8 << 8));  // no implicit copy assignment
static_assert(outer_door(^^Locked) == (0x3 | 0xB << 4 | 0xB << 8));
// Through the door again (the memoised specializations), and directly.
static_assert(outer_door(^^Pinned) == outer_v<^^Pinned>);
static_assert(inner_door(^^Locked) == inner_v<^^Locked>);

int main() {}
