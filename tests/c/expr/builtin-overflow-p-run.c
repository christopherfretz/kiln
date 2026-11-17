// GNU __builtin_{add,sub,mul}_overflow_p(a, b, c): whether a op b, computed
// in infinite precision, fits the type of c (its width, for a bit-field);
// c's value is not used, but its side effects happen. Operands of every
// width and sign, with values hidden from the folder (volatile) and
// folded; a signed multiplication wider than 64 bits; bit-field results.
// Compared with GCC 16.1 (clang 19/22 have no _p builtins). WP-1549.
//@ std: c17
//@ expect: run
//@ oracle: gcc
//@ skip(clang): clang 19/22 do not implement __builtin_*_overflow_p
//@ stdout: 0 1 1 0 1 0 1 0 0 1 1 0 0 0 1 1 0 1
//@ stdout: 0 1 1 0 1 0 1 0 0 1 1 0 0 0 1 1 0 1
//@ stdout: 1 1 1 0 1 1 0 1
//@ stdout: 1 1 0 7 0 1 0 0 1 0 0 0 0 1 0 0 1 1
int printf(const char *, ...);
struct B { int s3 : 3; unsigned u5 : 5; long long s40 : 40; } bf;
static volatile int vi;
static volatile long long vll;
static volatile unsigned vu;
static volatile unsigned long long vull;
static volatile signed char vsc;
static volatile __int128 v128;
#define I(x) (vi = (x), vi)
#define LL(x) (vll = (x), vll)
#define U(x) (vu = (x), vu)
#define ULL(x) (vull = (x), vull)
#define SC(x) (vsc = (x), vsc)
#define Q(x) (v128 = (x), v128)
#define ROW(F)                                                               \
  printf("%d %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d\n",          \
         __builtin_add_overflow_p(F(1), 2, (int)0),                          \
         __builtin_add_overflow_p(F(__INT_MAX__), 1, (int)0),               \
         __builtin_add_overflow_p(F(200), 100, (unsigned char)0),            \
         __builtin_add_overflow_p(F(-100), -28, (signed char)0),             \
         __builtin_add_overflow_p(F(-100), -29, (signed char)0),             \
         __builtin_sub_overflow_p(F(5), 3, 0u),                              \
         __builtin_sub_overflow_p(F(3), 5, 0u),                              \
         __builtin_sub_overflow_p(F(-1), __INT_MAX__, (int)0),               \
         __builtin_sub_overflow_p(F(-1), __INT_MAX__, 0LL),                  \
         __builtin_mul_overflow_p(F(-1), 1, 0u),                             \
         __builtin_mul_overflow_p(F(65536), 65536, (int)0),                  \
         __builtin_mul_overflow_p(F(65536), 65536, 0LL),                     \
         __builtin_mul_overflow_p(F(-65536), 32768, (int)0),                 \
         __builtin_mul_overflow_p(F(-65536), 32767, (int)0),                 \
         __builtin_add_overflow_p(F(3), 1, bf.s3),                           \
         __builtin_add_overflow_p(F(-4), -1, bf.s3),                         \
         __builtin_add_overflow_p(F(30), 1, bf.u5),                          \
         __builtin_add_overflow_p(F(31), 1, bf.u5))
#define NONE(x) (x)
int main(void) {
  ROW(NONE);
  ROW(I);
  // 64-bit and wider operands, run time only.
  printf("%d %d %d %d %d %d %d %d\n",
         __builtin_mul_overflow_p(LL(1) << 32, LL(1) << 31, 0LL),
         __builtin_mul_overflow_p(LL(1) << 32, LL(1) << 31, (__int128)0) + 1,
         __builtin_mul_overflow_p(ULL(~0ull), U(2), 0ull),
         __builtin_add_overflow_p(ULL(~0ull), 0, 0LL) - 1,
         __builtin_mul_overflow_p(Q((__int128)1 << 100), Q(1 << 30), (__int128)0),
         __builtin_mul_overflow_p(Q(-((__int128)1 << 62)), Q(-2), (long long)0),
         __builtin_mul_overflow_p(Q(-((__int128)1 << 62)), Q(2), (long long)0),
         __builtin_sub_overflow_p(SC(-128), LL(1), bf.s40) + 1);
  // Side effects of the result operand still happen.
  int y = 0;
  int r = __builtin_add_overflow_p(I(1), __INT_MAX__, y++);
  int s = __builtin_add_overflow_p(1, 2, bf.u5 = 7);
  printf("%d %d %d %d", r, y, s, bf.u5);
  printf(" %d %d %d %d %d %d %d %d %d %d %d %d %d %d\n",
         __builtin_add_overflow_p(1, 1, (char)0), __builtin_add_overflow_p(127, 1, (signed char)0),
         __builtin_add_overflow_p(1, 1, (short)0), __builtin_add_overflow_p(0, 0, 0ULL),
         __builtin_sub_overflow_p(0, 1, 0ULL), __builtin_sub_overflow_p(0u, 1u, 0),
         __builtin_mul_overflow_p(2, 3, (_Bool)0 + 0), __builtin_add_overflow_p(1, 1, 0L),
         __builtin_mul_overflow_p(-1, -1, 0u), __builtin_add_overflow_p(-1, -1, 0u),
         __builtin_add_overflow_p(1, 0, 0u), __builtin_add_overflow_p(0, -1, (long)0),
         __builtin_mul_overflow_p(1 << 15, 1 << 16, (int)0), __builtin_add_overflow_p(-8, 0, bf.s3));
  return 0;
}
