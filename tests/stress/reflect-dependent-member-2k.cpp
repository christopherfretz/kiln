// [expr.reflect]/7, [temp.dep.expr] (P2996R13): 2,000 instantiations of
// one dependent reflect-expression `^^T::x`, each for a distinct class
// C<N>, each compared with the non-dependent `^^C<N>::x` and queried
// (`type_of`). The dependent operand is resolved by substitution, never by
// a string re-lookup, so the work is linear in the instantiations; the
// binary split keeps the instantiation depth at 11
// (spec/landed/reflection/WP-519.md, "Performance").
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ expect: compile-ok
//@ oracle: gcc
//@ strict-diags
//@ oven: steps<=32000 calls<=11000 evaluations<=21000 peak_bytes<=64 allocs<=16
//@ mem<=1024
#if __has_include(<meta>)
#include <meta>
#else
#include "kiln_meta.h"
#endif
using std::meta::info;

template <int N> struct C { int x; static constexpr int n = N; };
template <class T> consteval info rx() { return ^^T::x; }
template <int N> consteval int check() {
  return rx<C<N>>() == ^^C<N>::x && std::meta::type_of(rx<C<N>>()) == ^^int;
}
template <int Lo, int Hi> consteval int run() {
  if constexpr (Hi - Lo == 1) {
    return check<Lo>();
  } else {
    return run<Lo, (Lo + Hi) / 2>() + run<(Lo + Hi) / 2, Hi>();
  }
}
static_assert(run<0, 2000>() == 2000);
static_assert(rx<C<0>>() != rx<C<1999>>());
