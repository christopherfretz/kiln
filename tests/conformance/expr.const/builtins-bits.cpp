// [expr.const], [bit.count], [bit.byteswap]: the bit-manipulation builtins
// are constant-evaluable (clang's constexpr builtins). The `g` forms take
// any unsigned integer type; their optional second argument is the result
// for a zero input.
//@ expect: compile-ok
//@ strict-diags
static_assert(__builtin_clz(1u) == 31);
static_assert(__builtin_clz(0x80000000u) == 0);
static_assert(__builtin_clzl(1ul) == 63);
static_assert(__builtin_clzll(0xffull) == 56);
static_assert(__builtin_ctz(8u) == 3);
static_assert(__builtin_ctzl(1ul << 40) == 40);
static_assert(__builtin_ctzll(0x8000000000000000ull) == 63);
static_assert(__builtin_popcount(0u) == 0);
static_assert(__builtin_popcount(0xf0f0u) == 8);
static_assert(__builtin_popcountl(~0ul) == 64);
static_assert(__builtin_popcountll(0x5555555555555555ull) == 32);

// The generic forms: any unsigned type, fallback value for zero.
static_assert(__builtin_clzg((unsigned char)1) == 7);
static_assert(__builtin_clzg((unsigned short)1) == 15);
static_assert(__builtin_clzg(0u, 42) == 42);
static_assert(__builtin_clzg((unsigned __int128)1) == 127);
static_assert(__builtin_ctzg((unsigned char)0, -1) == -1);
static_assert(__builtin_ctzg((unsigned long long)1 << 50) == 50);
static_assert(__builtin_popcountg((unsigned char)0xff) == 8);
static_assert(__builtin_popcountg(~(unsigned __int128)0) == 128);

// Byte swaps.
static_assert(__builtin_bswap16(0x1234) == 0x3412);
static_assert(__builtin_bswap32(0x12345678u) == 0x78563412u);
static_assert(__builtin_bswap64(0x0102030405060708ull) == 0x0807060504030201ull);

// Through constexpr functions, as libc++'s <bit> uses them.
constexpr int width(unsigned long long x) { return x ? 64 - __builtin_clzll(x) : 0; }
static_assert(width(0) == 0 && width(1) == 1 && width(255) == 8);
constexpr unsigned rotl(unsigned x, int s) { return (x << s) | (x >> (32 - s)); }
static_assert(__builtin_popcount(rotl(0x80000001u, 4)) == 2);
constexpr int sum_pop(const unsigned *p, int n) {
  int s = 0;
  for (int i = 0; i < n; ++i)
    s += __builtin_popcount(p[i]);
  return s;
}
constexpr unsigned words[] = {1, 3, 7, 15};
static_assert(sum_pop(words, 4) == 10);
// Result types follow clang: clz/ctz/popcount return int; bswap returns its operand type.
static_assert(sizeof(__builtin_bswap16(0)) == 2);
static_assert(sizeof(__builtin_bswap64(0)) == 8);
static_assert(sizeof(__builtin_clzll(1)) == sizeof(int));
