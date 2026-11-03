// [meta.reflection.traits], [expr.const] (spec/reflection-traits.md §3):
// a type trait is one query of the builtin trait engine with no heap
// allocation; it is `Stable`, so a consteval function reaching only it is
// memoized. 10,000 calls of a wrapper with the same argument are one
// evaluation and 9,999 memo hits (`memo_hits`), and allocate nothing
// (`allocs`); 10,000 direct calls in one loop allocate nothing either.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ oven: 32 memo_hits>=9999 allocs<=0
//@ oven: 33 allocs<=0
//@ oven: 34 allocs<=0
#include "kiln_meta_traits.h"
using namespace std::meta;

struct S { int x; };

consteval bool wrapped(info t) { return is_trivially_copyable_type(t); }

consteval int calls() {
  int n = 0;
  for (int i = 0; i < 10000; ++i) n += wrapped(^^S);
  return n;
}

consteval int direct() {
  int n = 0;
  for (int i = 0; i < 10000; ++i) n += is_class_type(^^S) + is_same_type(^^S, remove_cv(^^const S));
  return n;
}

static_assert(calls() == 10000);
static_assert(direct() == 20000);
static_assert(rank(^^int[1][2][3]) == 3);
