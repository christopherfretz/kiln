// [expr.const]/10, [meta.reflection.queries] (spec/reflection.md §3.3 and
// §3.6, the purity classes): a consteval function that reaches only `Stable`
// reflection intrinsics is as pure as one over integers, so its second
// call with the same `info` argument is answered from the memo table; one
// that reaches a `PointDependent` intrinsic (the answer may change as the
// program grows: the spike's members_of) is never memoized, and neither
// is the evaluation enclosing it.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ oven: 27 memo_hits<=0
//@ oven: 28 memo_hits>=1
//@ oven: 30 memo_hits<=0
//@ oven: 31 memo_hits<=0
#include "kiln_meta.h"
using namespace std::meta;

int v;
struct S {
  int a;
  char b;
};

consteval bool is_int(info r) { return type_of(r) == ^^int; }
consteval unsigned long count(info r) { return __kiln_members_count(r); }

static_assert(is_int(^^v));
static_assert(is_int(^^v));

static_assert(count(^^S) == 2);
static_assert(count(^^S) == 2);
