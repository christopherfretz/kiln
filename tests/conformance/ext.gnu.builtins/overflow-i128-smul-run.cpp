// GNU checked arithmetic: a signed multiplication wider than 64 bits
// (`__int128` operands, or an encompassing type of 129 bits) is expanded
// inline, so the program links against libgcc, which has no compiler-rt
// `__muloti4` (clang 19 calls it on aarch64). The wrapped product is
// stored and the flag says whether it did not fit (spec/codegen.md §3).
//@ expect: run
//@ oracle: none
//@ skip(clang): clang 19 lowers the i128 multiply to compiler-rt's __muloti4, absent from libgcc on aarch64
//@ stdout: 0 80000000000000000000000000000000
//@ stdout: 1 80000000000000000000000000000000
//@ stdout: 1 fffffffffffffffffffffffffffffffe
//@ stdout: 0 80000000000000000000000000000000
//@ stdout: 1 80000000000000000000000000000000
//@ stdout: 0 fffffffffffffffffffffffffffffff1
//@ stdout: 0 00000000000000000000000000000000
//@ stdout: 0 3fffffffffffffffffffffffffffffff
//@ stdout: 1 fffffffffffffffffffffffffffffffa
//@ stdout: 0 fffffffffffffffffffffffffffffffe
//@ stdout: 0 80000000000000008000000000000000
extern "C" int printf(const char *, ...);
template <class T> T opaque(T v) {
  volatile T x = v;
  return x;
}
static void show(bool o, unsigned __int128 v) {
  printf("%d %016llx%016llx\n", o, (unsigned long long)(v >> 64), (unsigned long long)v);
}
int main() {
  const __int128 max = (__int128)(~(unsigned __int128)0 >> 1), min = -max - 1;
  const __int128 two64 = (__int128)1 << 64, two63 = (__int128)1 << 63;
  __int128 r;
  unsigned __int128 u;
  bool o;
  o = __builtin_mul_overflow(opaque(min), opaque((__int128)1), &r); show(o, r);
  o = __builtin_mul_overflow(opaque(min), opaque((__int128)-1), &r); show(o, r);
  o = __builtin_mul_overflow(opaque(max), opaque((__int128)2), &r); show(o, r);
  o = __builtin_mul_overflow(opaque(-two64), opaque(two63), &r); show(o, r);
  o = __builtin_mul_overflow(opaque(two64), opaque(two63), &r); show(o, r);
  o = __builtin_mul_overflow(opaque((__int128)-3), opaque((__int128)5), &r); show(o, r);
  o = __builtin_mul_overflow(opaque((__int128)0), opaque(min), &r); show(o, r);
  o = __builtin_mul_overflow(opaque(-(max / 2)), opaque((__int128)-1), &r); show(o, r);
  // A 129-bit encompassing type (signed operands, unsigned result).
  o = __builtin_mul_overflow(opaque((__int128)-2), opaque((__int128)3), &u); show(o, u);
  o = __builtin_mul_overflow(opaque(max), opaque((__int128)2), &u); show(o, u);
  // LLONG_MIN * ULLONG_MAX into a 128-bit result (fits).
  o = __builtin_mul_overflow(opaque(-(long long)(~0ULL >> 1) - 1), opaque(~0ULL), &r); show(o, r);
  return 0;
}
