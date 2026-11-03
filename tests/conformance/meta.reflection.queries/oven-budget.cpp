// [meta.reflection.queries] (spec/reflection.md §3.3, the purity
// classes; spec/oven.md "Memoization"): a predicate answers from the
// reflectee in place, so 10,000 calls in a constant-evaluation loop
// allocate nothing in abstract memory (allocs), freeze nothing per call
// (frozen_bytes: the first loop's 16 are once per evaluation, not per
// call) and cost a constant number of steps each. A consteval function
// over Stable queries only is pure:
// its calls with the same info argument after the first are memo hits
// (memo_hits), 10,000 calls over 4 reflections giving 9,996 of them.
// One that asks a PointDependent query (is_complete_type, whose answer
// changes once the class is complete) is never memoized.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ oven: 51 steps<=40000 allocs<=0 frozen_bytes<=64 memo_hits>=9996
//@ oven: 52 steps<=40000 allocs<=0 frozen_bytes<=0 memo_hits<=0
//@ oven: 53 steps<=40000 allocs<=0 frozen_bytes<=0 memo_hits<=0
#include "kiln_meta_queries.h"
using namespace std::meta;

struct S {
  int m;
  static int sm;
  void f();
  struct In {};
};

consteval bool stable(info r) {
  return is_public(r) && is_class_member(r) && !has_linkage(r) == is_nonstatic_data_member(r) &&
         parent_of(r) == (^^S) && !is_namespace_member(r) && dealias(r) == r;
}
consteval bool point(info r) { return is_complete_type(r); }

constexpr info refl[4] = {^^S::m, ^^S::sm, ^^S::f, ^^S::In};

consteval int stable_loop() {
  int n = 0;
  for (int i = 0; i < 10000; ++i) n += stable(refl[i % 4]);
  return n;
}
consteval int point_loop() {
  int n = 0;
  for (int i = 0; i < 10000; ++i) n += point(refl[3]);
  return n;
}
consteval int direct_loop() {
  int n = 0;
  for (int i = 0; i < 10000; ++i) n += is_static_member(refl[i % 4]) + has_linkage(refl[i % 4]);
  return n;
}
static_assert(stable_loop() == 10000);
static_assert(point_loop() == 10000);
static_assert(direct_loop() == 2500 + 7500);
