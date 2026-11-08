// [expr.pre]/6, C23 7.12.2: under clang's C++ default `-ffp-contract=on`
// `a * a - c` within one expression is `llvm.fmuladd`, which AArch64
// fuses (`fmsub`): the result is the exact 2^-60, not 0 (WP-806).
//@ expect: run
//@ requires: target-aarch64
//@ stdout: expression 0x1p-60
#include <cstdio>

volatile double va = 1.0 + 0x1p-30, vc = 1.0 + 0x1p-29;

double expr(double a, double b, double c) { return a * b - c; }

int main() {
  double a = va, c = vc;
  std::printf("expression %a\n", expr(a, a, c));
}
