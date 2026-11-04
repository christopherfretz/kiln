// The multiprecision carry builtins at run time (clang LanguageExtensions
// "Multiprecision Arithmetic Builtins"; CGBuiltin BI__builtin_addcb..):
// T __builtin_{add,sub}cX(T x, T y, T carryin, T *carryout) for X = b, s,
// (none), l, ll is x + y + carryin (x - y - carryin) modulo 2^N, and
// *carryout is 1 when either wrapping step overflowed. Every width, with a
// carry-in of 0 and 1, with and without overflow, and a carry-in above 1.
// Verified against clang 19.
//@ expect: run
//@ stdout: 3:0,4:0,0:1,0:1,255:1,3:1,3:0,2:0,253:1,255:1,0:1,
//@ stdout: 3000:0,3001:0,0:1,0:1,1999:0,65535:1,65535:1,
//@ stdout: 4294967295:0,0:1,4294967295:1,6:0,4294967289:1,4294967295:1,
//@ stdout: 4294967296:0,0:1,0:1,0:0,18446744073709551615:1,
//@ stdout: 1311768467463790336:0,18446744073709551614:1,18446744073709551615:1,0:1,
//@ stdout: 18446744073709551615:1,0:0,0:1,99:0,
extern "C" int printf(const char *, ...);
template <class T> T opaque(T v) {
  volatile T x = v;
  return x;
}
typedef unsigned char uc; typedef unsigned short us; typedef unsigned u;
typedef unsigned long ul; typedef unsigned long long ull;
#define ADD(F, T, x, y, c)                                                   \
  do {                                                                       \
    T co = 7;                                                                \
    T r = F(opaque<T>(x), opaque<T>(y), opaque<T>(c), &co);                  \
    printf("%llu:%llu,", (ull)r, (ull)co);                                   \
  } while (0)
int main() {
  ADD(__builtin_addcb, uc, 1, 2, 0);
  ADD(__builtin_addcb, uc, 1, 2, 1);
  ADD(__builtin_addcb, uc, 0xff, 1, 0);
  ADD(__builtin_addcb, uc, 0xfe, 1, 1);
  ADD(__builtin_addcb, uc, 0xff, 0xff, 1);
  ADD(__builtin_addcb, uc, 0xfd, 1, 5);
  ADD(__builtin_subcb, uc, 5, 2, 0);
  ADD(__builtin_subcb, uc, 5, 2, 1);
  ADD(__builtin_subcb, uc, 2, 5, 0);
  ADD(__builtin_subcb, uc, 2, 2, 1);
  ADD(__builtin_subcb, uc, 0, 0xff, 1);
  printf("\n");
  ADD(__builtin_addcs, us, 1000, 2000, 0);
  ADD(__builtin_addcs, us, 1000, 2000, 1);
  ADD(__builtin_addcs, us, 0xffff, 1, 0);
  ADD(__builtin_addcs, us, 0xfffe, 1, 1);
  ADD(__builtin_subcs, us, 3000, 1000, 1);
  ADD(__builtin_subcs, us, 0, 1, 0);
  ADD(__builtin_subcs, us, 1, 1, 1);
  printf("\n");
  ADD(__builtin_addc, u, 0x80000000u, 0x7fffffffu, 0);
  ADD(__builtin_addc, u, 0x80000000u, 0x7fffffffu, 1);
  ADD(__builtin_addc, u, 0xffffffffu, 0xffffffffu, 1);
  ADD(__builtin_subc, u, 10, 3, 1);
  ADD(__builtin_subc, u, 3, 10, 0);
  ADD(__builtin_subc, u, 0, 0, 1);
  printf("\n");
  ADD(__builtin_addcl, ul, 0xfffffffful, 1, 0);
  ADD(__builtin_addcl, ul, ~0ul, 0, 1);
  ADD(__builtin_addcl, ul, ~0ul, 1, 0);
  ADD(__builtin_subcl, ul, 1, 0, 1);
  ADD(__builtin_subcl, ul, 0, 0, 1);
  printf("\n");
  ADD(__builtin_addcll, ull, 0x123456789abcdefull, 0x1111111111111111ull, 0);
  ADD(__builtin_addcll, ull, ~0ull, ~0ull, 0);
  ADD(__builtin_addcll, ull, ~0ull, ~0ull, 1);
  ADD(__builtin_addcll, ull, ~0ull - 1, 1, 1);
  printf("\n");
  ADD(__builtin_subcll, ull, 0, 1, 0);
  ADD(__builtin_subcll, ull, 1, 0, 1);
  ADD(__builtin_subcll, ull, 0, ~0ull, 1);
  ADD(__builtin_subcll, ull, 100, 1, 0);
  printf("\n");
}
