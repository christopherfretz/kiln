//@ std: c++23
//@ mode: run
//@ args: -O2
//@ threshold-time: 15
// std::generator<int> (libstdc++ <generator>) iterated 10M times by a
// range-for: one coroutine resumption, one co_yield and one iterator
// increment per element. Measures the generated code (the frame is
// heap-allocated once; CoroElide cannot apply because the generator
// escapes into the range-for's iterator). WP-370.
#include <generator>
#include <cstdio>

std::generator<int> iota(int n) {
  for (int i = 0; i < n; ++i)
    co_yield i;
}

int main(int argc, char**) {
  // argc keeps the bound opaque so the loop is not folded.
  int n = 10'000'000 * argc;
  unsigned long long sum = 0;
  for (int x : iota(n))
    sum += static_cast<unsigned>(x) ^ (sum >> 7);
  std::printf("%llu\n", sum);
  return sum == 0;
}
