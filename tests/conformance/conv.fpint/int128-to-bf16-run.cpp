// [conv.fpint]/2, [basic.extended.fp]: converting a 128-bit integer to
// `__bf16`. LLVM's AArch64 backend has no libcall for `sitofp`/`uitofp`
// from i128 to bfloat and aborts (clang 19 and 22 crash on it), so kiln
// calls libgcc's `__floattibf`/`__floatuntibf` there; x86-64 lowers the
// instruction itself. Found running GCC's C++ testsuite (WP-1456).
//@ expect: run
//@ skip(clang): clang 19/22 crash in the AArch64 backend on i128 -> bfloat
//@ strict-diags
//@ stdout: -1.26765e+30 3.98768e+36 7 1
#include <cstdio>
__bf16 s(__int128 x) { return (__bf16)x; }
__bf16 u(unsigned __int128 x) { return (__bf16)x; }
int main() {
  volatile __int128 a = -((__int128)1 << 100);
  volatile unsigned __int128 b = (unsigned __int128)3 << 120;
  volatile __int128 c = 7;
  std::printf("%g %g %g %d\n", (double)(float)s(a), (double)(float)u(b),
              (double)(float)s(c),
              (float)s(a) == -0x1p100f && (float)u(b) == 0x3p120f);
}
