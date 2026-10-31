// [basic.fundamental]: __int128 arithmetic at run time (not folded: the
// operands come through volatile objects), printed as two 64-bit halves.
//@ expect: run
//@ stdout: 4096 0
//@ stdout: 0 1 1
//@ stdout: 18446744073709551615 9223372036854775807
//@ stdout: 333 2049638230412172401
//@ stdout: -1 1 1267650600228229401496703205376
extern "C" int printf(const char*, ...);

static unsigned long long hi(__uint128_t v) { return (unsigned long long)(v >> 64); }
static unsigned long long lo(__uint128_t v) { return (unsigned long long)v; }

__int128_t mul(__int128_t a, __int128_t b) { return a * b; }

int main() {
  volatile long long one = 1;
  volatile int sh = 76;
  __uint128_t big = (__uint128_t)one << sh;
  printf("%llu %llu\n", hi(big), lo(big));
  __int128_t m = mul(-(__int128_t)one, big);
  printf("%d %d %d\n", (int)(m > 0), (int)(m < 0), (int)((__uint128_t)m > big));
  __uint128_t umax = ~(__uint128_t)(one - 1);
  printf("%llu %llu\n", hi(umax), lo(umax >> 65));
  __uint128_t q = umax / 3;
  printf("%llu %llu\n", (unsigned long long)(1000 / (__int128_t)(one * 3)), hi(q) / 3);
  __int128_t s = (__int128_t)(one << 50) * (one << 50);
  printf("%d %d %.0f\n", (int)(-s >> 127), (int)(s >> 100), (double)s);
}
