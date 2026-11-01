//@ args: -stdlib=libc++ -fconstexpr-steps=1000000000
// Transient allocation through std::allocator (spec/oven.md "Transient
// allocation", WP-212): build a constexpr std::vector<int> of 100000
// elements by push_back (geometric reallocation through
// std::allocator<int>::allocate/deallocate, construct_at, destroy_at), sum
// it, and let it be destroyed. Deallocated buffers are reclaimed during the
// evaluation, so peak_bytes is bounded by the last two buffers (old+new
// during the final reallocation), not by the sum of every buffer.
#include <vector>

constexpr long long build(int n) {
  std::vector<int> v;
  for (int i = 0; i < n; ++i) v.push_back(i);
  long long sum = 0;
  for (int x : v) sum += x;
  return sum;
}

static_assert(build(100000) == 100000LL * 99999 / 2);
