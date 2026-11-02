// [expr.add]/1, [expr.ass]/6: `a + b*c`, `a - b*c` and the compound forms
// keep the sign of each operand when the product is contracted into a
// fused multiply-add (-ffp-contract=on, [expr.pre]/6 latitude); libc++
// `generate_canonical` accumulates `__sp += (__g() - min) * __base`.
//@ expect: run
//@ stdout: 21 -19 21 19 -19 -19 21 -21 | 21 -19 23 | 0.085032448717433665
extern "C" int printf(const char *, ...);
int main(int argc, char **) {
  double a = argc, b = 2, c = 10;
  double r[] = {a + b * c,    a - b * c,    b * c + a,     b * c - a,
                -(b * c) + a, a + -(b * c), a - -(b * c), -(b * c) - a};
  double s1 = a;
  s1 += b * c;
  double s2 = a;
  s2 -= b * c;
  unsigned long x = 10 + argc;
  double s3 = a;
  s3 += x * b;
  for (double v : r)
    printf("%g ", v);
  printf("| %g %g %g | ", s1, s2, s3);
  // generate_canonical<double, 53>(minstd_rand) with its first two draws.
  unsigned long g1 = 48271, g2 = 182605794, min = 1;
  double base = 2147483646.0, sp = g1 - min;
  sp += (g2 - min) * base;
  base *= 2147483646.0;
  printf("%.17g\n", sp / base);
}
