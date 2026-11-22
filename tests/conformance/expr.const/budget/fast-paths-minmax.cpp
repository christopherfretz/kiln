// [implimits]: a call of std::min of two scalars costs one step in the
// Oven (spec/oven.md "Library fast paths"): this loop needs 205 steps on
// kiln (405 running libstdc++'s std::min).
//@ args: -fconstexpr-steps=205
//@ expect: compile-ok
//@ skip(clang): clang counts steps by its own rule
#include <algorithm>
constexpr int low(int n) {
  int m = n;
  for (int i = 0; i < n; ++i)
    m = std::min(m, i);
  return m;
}
static_assert(low(100) == 0);
