// [meta.reflection.result], spec/reflection-values.md §3 and spec/oven.md
// "Reflection bytes": reflect_constant interns its result for the
// translation unit, so 10,000 calls over 16 distinct values make 16
// reflections and 16 constants. refl_bytes counts what the calls added to
// the interned tables (each entry once: a repeated call finds its result
// and adds nothing), and nothing is allocated on the constexpr heap.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
//@ oven: 26 refl_bytes<=720 refl_bytes>=1 allocs<=0 steps<=60000
#include "kiln_meta.h"
using namespace std::meta;

consteval int run() {
  int hits = 0;
  info seen[16] = {};
  for (int i = 0; i < 10000; ++i) {
    info r = reflect_constant(i % 16);
    if (i < 16) seen[i] = r;
    else if (seen[i % 16] != r) return -1;
    hits += extract<int>(r) == i % 16;
  }
  return hits;
}
static_assert(run() == 10000);
