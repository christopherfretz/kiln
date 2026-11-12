// C23 6.5.9, 6.3.1.3, 6.2.5 (_BitInt): (unsigned _BitInt(1))24 is 0, so
// `(unsigned _BitInt(1))2 < (unsigned _BitInt(1))24 << 0` is false. In an
// `optnone` function at -O2 the AArch64 backend selects with FastISel,
// which compares the whole register for an unsigned compare of i1 values
// made by a shift or a multiply (LLVM 19 and 22; found by the hod project,
// github.com/christopherfretz/hod, tests/ir/selector_i1_compare.ll). The
// IR is clang's, and clang's program is wrong the same way, so kiln does
// not work around it (docs/KNOWN_ISSUES.md "LLVM code generator bugs").
//@ expect: run
//@ std: c23
//@ args: -O2
//@ requires: target-aarch64
//@ stdout: 0 0 0
//@ xfail: LLVM FastISel on AArch64 compares the whole register for an unsigned compare of i1 (llc 19 and 22, as clang)
int printf(const char *, ...);
#define B1 unsigned _BitInt(1)
int x2 = 2, x24 = 24;
__attribute__((noinline, optnone)) int shl0(int a, int b) { return (B1)a < (B1)((B1)b << 0); }
__attribute__((noinline, optnone)) int mul1(int a, int b) { return (B1)a < (B1)((B1)b * (B1)1); }
__attribute__((noinline, optnone)) int branch(int b) {
  if ((B1)0 < (B1)((B1)b >> 0))
    return 1;
  return 0;
}
int main(void) {
  printf("%d %d %d\n", shl0(x2, x24), mul1(x2, x24), branch(x24));
  return 0;
}
