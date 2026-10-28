// [expr.const]/5.1, [implimits]: insertion sort of 64 LCG values through
// pointer and reference parameters; 5855 is the smallest
// -fconstexpr-steps clang 19 accepts (sort-over-limit.cpp checks 5854 fails
// on both compilers).
//@ args: -fconstexpr-steps=5855
//@ oven: 37 steps<=5855 calls<=1018 bytes<=512
//@ oven: 38 steps<=5785 calls<=1004 bytes<=512
constexpr void lcg_fill(unsigned *a, int n, unsigned seed) {
  for (int i = 0; i < n; ++i) {
    seed = seed * 1664525u + 1013904223u;
    a[i] = seed >> 8;
  }
}
constexpr void swap(unsigned &x, unsigned &y) {
  unsigned t = x;
  x = y;
  y = t;
}
constexpr void insertion_sort(unsigned *a, int n) {
  for (int i = 1; i < n; ++i)
    for (int j = i; j > 0 && a[j - 1] > a[j]; --j) swap(a[j - 1], a[j]);
}
constexpr bool check(unsigned seed) {
  unsigned in[64] = {};
  unsigned a[64] = {};
  lcg_fill(in, 64, seed);
  lcg_fill(a, 64, seed);
  insertion_sort(a, 64);
  unsigned long long sum_in = 0, sum_out = 0;
  for (int i = 0; i < 64; ++i) {
    sum_in += in[i];
    sum_out += a[i];
    if (i > 0 && a[i - 1] > a[i]) return false;
  }
  return sum_in == sum_out;
}
static_assert(check(1));
static_assert(check(42));
