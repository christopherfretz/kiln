// [expr.pre]/6, C23 7.12.2: `-ffp-contract=off` forbids contraction even
// within one expression: `a * a - c` rounds the product and is 0 where
// the fused result is 2^-60 (WP-806).
//@ expect: run
//@ args: -ffp-contract=off -O2
//@ stdout: expression 0x0p+0
//@ stdout: fma 0x1p-60
#include <cmath>
#include <cstdio>

volatile double va = 1.0 + 0x1p-30, vc = 1.0 + 0x1p-29;

double expr(double a, double b, double c) { return a * b - c; }

int main() {
  double a = va, c = vc;
  std::printf("expression %a\n", expr(a, a, c));
  std::printf("fma %a\n", std::fma(a, a, -c));
}
