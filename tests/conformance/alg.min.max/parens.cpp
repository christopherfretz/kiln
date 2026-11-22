// [alg.min.max], [expr.prim.paren]: a parenthesised call of std::min or
// std::max read for its value is the same call. The Oven's native
// std::min (spec/oven.md "Library fast paths") looked through the
// parentheses to take the arguments and then expected a bare call
// (WP-2061, reduced from real code: a shift count folded for
// -Wshift-count-overflow).
//@ expect: compile-ok
//@ strict-diags
#include <algorithm>
#include <cstddef>
unsigned long long bits(std::size_t n) {
  return 1ull << (std::min((std::size_t)31, n));
}
constexpr int pick(int a, int b) {
  int lo = (std::min(a, b));
  int hi = ((std::max(a, b)));
  return lo * 10 + hi;
}
static_assert(pick(3, 5) == 35);
static_assert(pick(5, 3) == 35);
constexpr bool same() {
  int a = 2, b = 2;
  return &(std::min(a, b)) == &a && (std::max(a, b)) == 2;
}
static_assert(same());
