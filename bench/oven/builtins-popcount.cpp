//@ args: -fconstexpr-steps=200000000
// The bit builtins inside constant evaluation, as libc++'s <bit> and bitset
// use them: popcount/clz/ctz/bswap over a long run of 32- and 64-bit values
// (WP-088). Each builtin is one Oven instruction.

constexpr unsigned long long mix(int n) {
  unsigned long long x = 0x9e3779b97f4a7c15ull, acc = 0;
  for (int i = 0; i < n; ++i) {
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    acc += __builtin_popcountll(x) + __builtin_popcount((unsigned)x);
    acc += __builtin_clzll(x | 1) + __builtin_ctzll(x | (1ull << 63));
    acc ^= __builtin_bswap64(acc);
    acc += __builtin_popcountg((unsigned short)x) + __builtin_clzg((unsigned char)x, 8);
  }
  return acc;
}

static_assert(mix(0) == 0);
static_assert(mix(200000) == 13618885273168380103ull);
