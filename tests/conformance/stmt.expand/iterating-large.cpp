// [stmt.expand]/5.2 (P1306R5): an iterating expansion over a 100000-element
// constexpr std::array, counted by one `end - begin` evaluation rather
// than 100000 comparisons (spec/sema.md §7.6a; docs/status/perf-history.md
// "WP-454" records wall time and peak memory).
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang>=23
#include <array>
constexpr std::array<int, 100000> big = [] {
  std::array<int, 100000> a{};
  for (int i = 0; i < 100000; ++i) a[i] = i % 7;
  return a;
}();
constexpr long sum() {
  long s = 0;
  template for (constexpr int x : big) { s += x; }
  return s;
}
static_assert(sum() == 299995);
