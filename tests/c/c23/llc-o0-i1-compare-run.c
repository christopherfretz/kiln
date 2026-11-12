// C23 6.5.9 (relational operators), 6.3.1.3, 6.2.5 (_BitInt): an
// unsigned _BitInt(1) has the value of the low bit of what it was
// converted from, and compares by that value. The shapes whose IR is an
// unsigned `icmp` of an i1 made by a shift or a multiply (clang's IR has
// them too) are miscompiled by LLVM 19/22 FastISel on AArch64, found by
// the hod project (github.com/christopherfretz/hod,
// tests/ir/selector_i1_compare.ll); at -O0 kiln, like clang, reaches
// GlobalISel, which is right, so these pass. FastISel is reached by an
// `optnone` function at -O1 and above: llc-o2-optnone-i1-compare-run.c
// (xfail), docs/KNOWN_ISSUES.md "LLVM code generator bugs". Comparisons
// of bool, of 1-bit bit-fields and of comparison results promote to int
// and are not affected.
//@ expect: run
//@ std: c23
//@ stdout: 0 0 0 0 0 0 0 0
//@ stdout: 0 0 0 0 0 1 1 0 0 0
#include <stdbool.h>
int printf(const char *, ...);
#define B1 unsigned _BitInt(1)
int x2 = 2, x24 = 24, x26 = 26, x27 = 27;
struct B { unsigned b : 1; unsigned c : 1; unsigned pad : 6; } gs = {0, 0, 0x3f};
bool ba = false, bb = true;
B1 u1a = 0, u1b = 1;
__attribute__((noinline)) int shl0(int a, int b) { return (B1)a < (B1)((B1)b << 0); }
__attribute__((noinline)) int shr0(int a, int b) { return (B1)a < (B1)((B1)b >> 0); }
__attribute__((noinline)) int mul1(int a, int b) { return (B1)a < (B1)((B1)b * (B1)1); }
__attribute__((noinline)) int shl_var(int a, int b) {
  B1 x = (B1)a, y = (B1)b;
  return x < (B1)(y << x);
}
__attribute__((noinline)) int mul_shl(int a, int b) { return (B1)((B1)a * (B1)b) < (B1)((B1)b << 0); }
__attribute__((noinline)) int branch(int b) {
  if ((B1)0 < (B1)((B1)b << 0))
    return 1;
  return 0;
}
__attribute__((noinline)) int compound_shl(int b) {
  B1 y = (B1)b;
  y <<= 0;
  return (B1)0 < y;
}
__attribute__((noinline)) int compound_mul(int b) {
  B1 y = (B1)b;
  y *= (B1)1;
  return (B1)0 < y;
}
__attribute__((noinline)) int truncs(int a, int b) { return (B1)a < (B1)b; }
__attribute__((noinline)) int bools(int a, int b) { return (bool)a < (bool)b; }
__attribute__((noinline)) int compares(int a, int b, int c, int d) { return (a < b) < (c < d); }
__attribute__((noinline)) int bitfields(void) { return gs.b < gs.c; }
__attribute__((noinline)) int bool_vars(void) { return ba < bb; }
__attribute__((noinline)) int bitint_vars(void) { return u1a < u1b; }
__attribute__((noinline)) int locals(int a, int b) {
  B1 x = a, y = b;
  return x < y;
}
__attribute__((noinline)) int ge(int a, int b) { return (B1)a >= (B1)b; }
int main(void) {
  printf("%d %d %d %d %d %d %d %d\n", shl0(x2, x24), shr0(x2, x24), mul1(x2, x24), shl_var(x2, x24),
         mul_shl(x2, x24), branch(x24), compound_shl(x24), compound_mul(x24));
  printf("%d %d %d %d %d %d %d %d %d %d\n", truncs(x2, x24), bools(x26, x27), compares(1, 2, 3, 4), bitfields(),
         compares(2, 1, 4, 3), bool_vars(), bitint_vars(), locals(x2, x24), ge(x2, x24) - 1, truncs(x26, x27) - 1);
  return 0;
}
