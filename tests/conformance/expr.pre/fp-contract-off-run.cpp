// [expr.pre]/6, C23 7.12.2: a floating-point expression may be contracted
// (evaluated as if exactly, `fma`) only where contraction is allowed.
// `#pragma STDC FP_CONTRACT OFF` in a scope, or a product split across two
// statements, rounds the product first: `a * a - c` is 0 where the fused
// result is 2^-60 (WP-806).
//@ expect: run
//@ stdout: off 0x0p+0
//@ stdout: two statements 0x0p+0
//@ stdout: fma 0x1p-60
#include <cmath>
#include <cstdio>

volatile double va = 1.0 + 0x1p-30, vc = 1.0 + 0x1p-29;

double off(double a, double b, double c) {
#pragma STDC FP_CONTRACT OFF
  return a * b - c;
}

double two(double a, double b, double c) {
  double p = a * b;
  return p - c;
}

int main() {
  double a = va, c = vc;
  std::printf("off %a\n", off(a, a, c));
  std::printf("two statements %a\n", two(a, a, c));
  std::printf("fma %a\n", std::fma(a, a, -c));
}
