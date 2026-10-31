// [expr.const]: __int128 arithmetic in constant evaluation (two's
// complement, unsigned wrap-around, conversions to and from floating).
//@ expect: compile-ok
constexpr __int128_t big = (__int128_t)1 << 100;
constexpr __uint128_t umax = ~(__uint128_t)0;
constexpr __int128_t imax = (__int128_t)(umax >> 1);
constexpr __int128_t imin = -imax - 1;
static_assert(big / 3 * 3 + big % 3 == big);
static_assert(-big / 7 == -(big / 7) && -big % 7 == -(big % 7));
static_assert(big >> 99 == 2 && (-big >> 100) == -1);
static_assert((umax >> 127) == 1 && (umax << 127) == (__uint128_t)1 << 127);
static_assert(umax + 1 == 0 && (__uint128_t)0 - 1 == umax);
static_assert(umax % 10 == 5 && umax / ((__uint128_t)1 << 64) == 0xFFFF'FFFF'FFFF'FFFFULL);
static_assert(imin < 0 && imax > 0 && imin + imax == -1);
static_assert((__uint128_t)imin == (__uint128_t)1 << 127);
static_assert((__int128_t)-1 < 0 && (__uint128_t)-1 > 0);
static_assert((long long)(big + 5) == 5 && (unsigned)(umax) == 0xFFFFFFFFu);
static_assert((double)big == 1267650600228229401496703205376.0);
static_assert((__int128_t)1e30 == (__int128_t)1000000000000000019884624838656.0);
static_assert((double)umax == 340282366920938463463374607431768211455.0);
static_assert((__int128_t)-2.5 == -2 && (__uint128_t)3.99 == 3);
static_assert((big & -big) == big && (big | 1) - big == 1 && (big ^ big) == 0);
static_assert(__builtin_popcountg(umax) == 128 && __builtin_ctzg((__uint128_t)big) == 100);
static_assert(__builtin_clzg((__uint128_t)1) == 127);
constexpr __int128_t fact(int n) { return n <= 1 ? 1 : n * fact(n - 1); }
static_assert(fact(33) / fact(32) == 33);
static_assert(fact(33) % 1000000007 == fact(33) - fact(33) / 1000000007 * 1000000007);
constexpr __uint128_t pow10(int n) {
  __uint128_t r = 1;
  while (n-- > 0) r *= 10;
  return r;
}
static_assert(pow10(38) / pow10(19) == pow10(19));
static_assert(pow10(38) > (__uint128_t)imax / 2);
