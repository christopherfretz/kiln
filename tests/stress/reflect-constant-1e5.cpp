// spec/reflection-values.md §6, spec/oven.md "Limits and metrics"
// (Reflection bytes), WP-479 (D5): 10^5 reflect_constant calls per
// evaluation. What a call adds to the translation unit's reflection table
// and constant store is the evaluation's `refl_bytes`, part of
// `peak_bytes`; content interning bounds it by the number of DISTINCT
// values, not of calls.
// - line 27: 10^5 calls over 16 values: 16 table entries.
// - line 28: 10^5 distinct values: linear (about 45 bytes each).
// - line 29: the same again: everything is interned already (and the
//   call is a memo hit), so nothing is charged.
// reflect-constant-1e5-limit.cpp is the same growth under a small
// -fconstexpr-memory.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ expect: compile-ok
//@ oven: 27 refl_bytes<=720 refl_bytes>=1 peak_bytes<=800 steps<=200005
//@ oven: 28 refl_bytes<=4500000 refl_bytes>=4000000 peak_bytes<=4500016 frozen_bytes<=1600000
//@ oven: 29 refl_bytes<=0 frozen_bytes<=0 memo_hits>=1
//@ mem<=256
#include "kiln_meta.h"
consteval int run(int n, int k, int base) {
  int hits = 0;
  for (int i = 0; i < n; ++i)
    if (std::meta::reflect_constant(base + i % k) == std::meta::reflect_constant(base + i % k)) ++hits;
  return hits;
}
static_assert(run(100000, 16, 1000000) == 100000);
static_assert(run(100000, 100000, 0) == 100000);
static_assert(run(100000, 100000, 0) == 100000);
