// [allocator.members], [expr.const]/5.20 + kiln Oven budgets (spec/oven.md
// "Transient allocation"): a std::vector<int> of 10000 elements built by
// push_back reallocates through std::allocator<int>; each old buffer is
// reclaimed when deallocated, so the heap peaks at the last two buffers
// (8192 + 16384 ints = 96 KiB during the final reallocation), not at the
// sum of every buffer. The 10^5-element version is
// bench/oven/vector-alloc.cpp.
//@ args: -stdlib=libc++ -fconstexpr-steps=100000000
//@ oven: 21 allocs<=15 peak_bytes<=106496 peak_arena<=163840
#include <vector>

constexpr long long build(int n) {
  std::vector<int> v;
  for (int i = 0; i < n; ++i) v.push_back(i);
  long long sum = 0;
  for (int x : v) sum += x;
  return sum;
}

// sum of 0..9999
static_assert(build(10000) == 10000LL * 9999 / 2);
