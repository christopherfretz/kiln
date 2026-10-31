// __int128 through the implicit typedefs __int128_t / __uint128_t:
// division, remainder, comparisons, shifts, conversions to and from
// floating and narrower integer types, and calls where the 128-bit
// argument is not first (AArch64: even register pair; x86-64: an
// INTEGER pair, on the stack once the six registers run out).
__int128_t sdiv(__int128_t a, __int128_t b) { return a / b; }
__uint128_t udiv(__uint128_t a, __uint128_t b) { return a / b; }
__int128_t srem(__int128_t a, __int128_t b) { return a % b; }
__uint128_t urem(__uint128_t a, __uint128_t b) { return a % b; }
__int128_t sshr(__int128_t a, int n) { return a >> n; }
__uint128_t ushr(__uint128_t a, unsigned n) { return a >> n; }
__int128_t neg(__int128_t a) { return -a; }
__uint128_t bits(__uint128_t a, __uint128_t b) { return (a & b) | (a ^ ~b); }
bool lt(__int128_t a, __int128_t b) { return a < b; }
bool ule(__uint128_t a, __uint128_t b) { return a <= b; }
bool eq(__int128_t a, __uint128_t b) { return a == b; }
double to_double(__int128_t a) { return a; }
float u_to_float(__uint128_t a) { return a; }
__int128_t from_double(double d) { return d; }
__uint128_t from_float(float f) { return f; }
int narrow(__int128_t a) { return a; }
__uint128_t widen(unsigned long long a, int b) { return a + b; }
bool truth(__int128_t a) { return a; }
__int128_t inc(__int128_t &a) { return a++ + ++a; }
__int128_t after_int(int x, __int128_t a) { return a + x; }
__int128_t many(long a, long b, long c, long d, long e, __int128_t f, __int128_t g) {
  return a + b + c + d + e + f + g;
}
__int128_t call_many() {
  __int128_t v = (__int128_t)1 << 90;
  return many(1, 2, 3, 4, 5, v, v + 1) + after_int(7, v);
}
__int128_t global_i = ((__int128_t)1 << 100) + 7;
__uint128_t global_u = ~(__uint128_t)0;
