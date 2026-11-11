// C23 6.6p5, 6.7.11p4 (Annex F.8.4): a floating constant expression in an
// initializer of an object with static storage duration is evaluated as at
// translation time with IEEE arithmetic, so a division by zero and an
// invalid operation give an infinity and a NaN, as clang folds them
// (`1.0 / 0.0`, `1.0 / 0.0 - 1.0 / 0.0`, `HUGE_VAL`, `NAN`), in a scalar, an
// aggregate and a block-scope static. (C++ keeps rejecting a division by
// zero in a constant expression, [expr.const]/10.7.)
//@ expect: run
//@ stdout: inf 1 1 1 1 1 | 1 1 1 1 | 1 1
#include <math.h>
#include <stdio.h>
double pinf = 1.0 / 0.0;
double dnan = 1.0 / 0.0 - 1.0 / 0.0;
float fnan = 0.0f / 0.0f;
long double lnan = 0.0L / 0.0L;
double huge = HUGE_VAL;
float qnan = NAN;
static const double mzero = -0.0;
struct T { double a, b; float c; long double d; } t[] = {
  { 0.0 / 0.0, -1.0 / 0.0, 1.0f / 0.0f, -(0.0L / 0.0L) },
};
int main(void) {
  static double q = -1.0 / 0.0;
  static double r[2] = { 1.0 / 0.0 - 1.0 / 0.0, 1.0 / -0.0 };
  printf("%g %d %d %d %d %d | ", pinf, isnan(dnan) != 0, isnan(fnan) != 0,
         isnan(lnan) != 0, huge == pinf, isnan(qnan) != 0);
  printf("%d %d %d %d | ", isnan(t[0].a) != 0, t[0].b == -pinf, isinf(t[0].c) != 0,
         isnan(t[0].d) != 0);
  printf("%d %d\n", q == -pinf && signbit(mzero), isnan(r[0]) && r[1] == -pinf);
  return 0;
}
