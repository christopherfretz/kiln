// [c.math.fpclass], [bit.count], [cstring.syn], [cwchar.syn]: the bit,
// memory, string and floating-point builtins at run time (operands read from
// volatiles, so nothing folds) agree with clang.
//@ expect: run
//@ stdout: 8 12 8
//@ stdout: 0 0 2
//@ stdout: -1 6 16
//@ stdout: 3412 f0f000 100000000000080
//@ stdout: kiln constexpr 14
//@ stdout: kikiln cstexpr
//@ stdout: 1 0
//@ stdout: 5 1
//@ stdout: 11 5 1
//@ stdout: 1 1 1 0
//@ stdout: 1 0 1
//@ stdout: 1 1 0 1 0 1
//@ stdout: 1 1
//@ stdout: 0.75 6 6 0.75
//@ stdout: 1024 2 1 16
//@ stdout: 2.718282 1.000000
extern "C" int printf(const char *, ...);
typedef decltype(sizeof 0) size_t;
volatile unsigned vu = 0x00f0f000u, vzero = 0;
volatile unsigned long long vull = 0x8000000000000001ull;
volatile unsigned char vuc = 0x40;
volatile unsigned short vus = 0xffff, vus2 = 0x1234;
volatile size_t n15 = 15, n6 = 6, n14 = 14;
volatile double vone = 1.0, vzerod = 0.0, vsub = 1e-310, v48 = 48.0, v075 = 0.75, v15 = 1.5, v10 = 10.0, v2 = 2.0, vtiny = 1e-10;
volatile float vzerof = 0.0f;
volatile long double vonel = 1.0L;
int main() {
  unsigned u = vu;
  unsigned long long ull = vull;
  printf("%d %d %d\n", __builtin_clz(u), __builtin_ctz(u), __builtin_popcount(u));
  printf("%d %d %d\n", __builtin_clzll(ull), __builtin_ctzll(ull), __builtin_popcountll(ull));
  printf("%d %d %d\n", __builtin_clzg((unsigned)vzero, -1), __builtin_ctzg((unsigned char)vuc), __builtin_popcountg((unsigned short)vus));
  printf("%x %x %llx\n", (unsigned)__builtin_bswap16(vus2), __builtin_bswap32(u), (unsigned long long)__builtin_bswap64(ull));

  char src[16] = "kiln constexpr";
  char dst[16] = {};
  __builtin_memcpy(dst, src, n15);
  printf("%s %zu\n", dst, __builtin_strlen(dst));
  __builtin_memmove(dst + 2, dst, n6);
  printf("%s\n", dst);
  printf("%d %d\n", __builtin_memcmp(src, dst, 2) == 0, __builtin_memcmp(src, dst, 6) < 0);
  const char *hit = (const char *)__builtin_memchr(src, 'c', n14);
  printf("%td %d\n", hit - src, __builtin_char_memchr(src, 'z', n14) == nullptr);
  const wchar_t *w = L"wide string";
  printf("%zu %td %d\n", __builtin_wcslen(w), __builtin_wmemchr(w, L's', 11) - w, __builtin_wmemcmp(w, L"wider", 5) < 0);

  double d = vone, z = vzerod;
  double inf = d / z, nan = z / z, sub = vsub;
  printf("%d %d %d %d\n", __builtin_isinf(inf), __builtin_isnan(nan), __builtin_isfinite(sub), __builtin_isnormal(sub));
  printf("%d %d %d\n", __builtin_signbit(-z) != 0, __builtin_signbit(d) != 0, __builtin_isnormal(d));
  printf("%d %d %d %d %d %d\n", __builtin_isgreater(d, z), __builtin_isgreaterequal(d, d), __builtin_isless(nan, d),
         __builtin_islessequal(z, d), __builtin_islessgreater(nan, d), __builtin_isunordered(nan, d));
  printf("%d %d\n", __builtin_isinf(__builtin_huge_valf()), __builtin_isnan(__builtin_nanl("")));
  int e;
  double m = __builtin_frexp(v48, &e);
  printf("%g %d %g %g\n", m, e, __builtin_ldexp(v075, 3), __builtin_scalbn(v15, -1));
  printf("%g %g %g %g\n", __builtin_exp2(v10), __builtin_pow(v2, 0.5) * __builtin_pow(2.0, 0.5),
         __builtin_expf(vzerof), (double)__builtin_scalblnl(vonel, 4L));
  printf("%.6f %.6f\n", __builtin_exp(vone), __builtin_expm1(vtiny) * 1e10);
}
