// [implimits]: a std::min call the Oven would evaluate natively
// (spec/oven.md "Library fast paths") with no step left declines, so the
// step limit fires inside the library's call, with its note.
//@ args: -fconstexpr-steps=203
//@ expect: compile-fail
//@ skip(clang): clang counts steps by its own rule
//@ diag: 17:15 error "static assertion expression is not an integral constant expression"
//@ diag: 14:9 note "in call to 'min<int>(m, i)'"
//@ diag: 17:15 note "in call to 'low(100)'"
#include <algorithm>
constexpr int low(int n) {
  int m = n;
  for (int i = 0; i < n; ++i)
    m = std::min(m, i);
  return m;
}
static_assert(low(100) == 0);
