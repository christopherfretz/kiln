// spec/reflection.md §3.9, §3.5: the names family allocates nothing on
// the constexpr heap. Each (reflection, string kind) is computed and
// frozen once per translation unit, so 10,000 identifier_of calls over a
// handful of reflections freeze no more static bytes than one call per
// reflection does.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
//@ oven: 31 allocs<=0
//@ oven: 32 allocs<=0
//@ oven: 33 allocs<=0
//@ oven: 34 allocs<=0 frozen_bytes<=256 static_bytes<=256 steps<=80000
//@ oven: allocs<=0 static_bytes<=512
#include "kiln_meta.h"
using namespace std::meta;

struct Alpha {
  int beta;
  void gamma();
};
consteval unsigned long loop(int n) {
  info rs[3] = {^^Alpha, ^^Alpha::beta, ^^Alpha::gamma};
  unsigned long total = 0;
  for (int i = 0; i < n; ++i) {
    total += identifier_of(rs[i % 3]).size();
    total += u8identifier_of(rs[i % 3]).size();
  }
  return total;
}
static_assert(identifier_of(^^Alpha) == "Alpha");
static_assert(display_string_of(^^Alpha::beta) == "Alpha::beta");
static_assert(source_location_of(^^Alpha).line() == 18);
static_assert(loop(10000) == 2 * (3334 * 5 + 3333 * 4 + 3333 * 5));

int main() {}
