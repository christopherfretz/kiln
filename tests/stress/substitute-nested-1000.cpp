// [meta.reflection.substitute] (P2996R13), spec/reflection.md §3.3: the
// PointDependent bridge re-enters sema with the Oven lent (WP-482 item 5).
// W<T>'s constraint calls can_substitute(^^W, {U}) for T = Box<U>, so
// asking whether W<Box^1000<int>> is valid nests 1,000 substitutions, each
// inside the evaluation of the previous one's constraint: no second Oven,
// no re-entrant borrow. The chain itself is built by 1,000 chained
// substitute calls in one consteval function. Each nested constraint
// evaluation is its own evaluation on the lent Oven (1,000 of them, each
// reclaiming its template_arguments_of vector). The depth flags are for
// both compilers: kiln counts constexpr call depth across the nested
// evaluations (two calls per level), GCC its template depth.
//@ args: -freflection -I tests/support -fconstexpr-depth=4096 -ftemplate-depth=4096
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ expect: compile-ok
//@ oracle: gcc
//@ strict-diags
// refl_bytes is linear in the depth: each level's new reflections are
// charged once, by the call that interned them, not again by every
// enclosing call (spec/oven.md "Reflection bytes").
//@ oven: peak_bytes<=29200 refl_bytes<=58100 allocs<=1100 evaluations<=1100 steps<=20000
#if __has_include(<meta>)
#include <meta>
#else
#include "kiln_meta.h"
#endif
using namespace std::meta;

template <class T> struct Box {};

consteval info boxes(int n) {
  info r = ^^int;
  for (int k = 0; k < n; ++k) r = substitute(^^Box, {r});
  return r;
}

consteval bool deep(info t);
template <class T> requires(deep(^^T)) struct W {};
consteval bool deep(info t) {
  if (t == ^^int) return true;
  return can_substitute(^^W, {template_arguments_of(t)[0]});
}

constexpr info b1000 = boxes(1000);
static_assert(b1000 != boxes(999));
static_assert(template_arguments_of(b1000)[0] == boxes(999));
static_assert(can_substitute(^^W, {b1000}));
static_assert(substitute(^^W, {b1000}) != ^^W<int>);

int main() {}
