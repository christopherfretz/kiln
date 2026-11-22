// [implimits]: a call of std::is_constant_evaluated (libstdc++ defines it
// with if consteval) costs no step in the Oven, as clang's std::move does
// (spec/oven.md "Library fast paths"): this loop needs 105 steps on kiln,
// 505 on clang, which runs the wrapper's statements.
//@ args: -fconstexpr-steps=105
//@ expect: compile-ok
//@ skip(clang): clang charges the wrapper's four statements per call
#include <type_traits>
constexpr int count(int n) {
  int k = 0;
  for (int i = 0; i < n; ++i)
    k += std::is_constant_evaluated();
  return k;
}
static_assert(count(100) == 100);
