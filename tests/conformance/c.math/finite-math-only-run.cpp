// C23 7.12.2, [cfenv.syn]: `-ffinite-math-only` defines
// `__FINITE_MATH_ONLY__` to 1 and puts `nnan ninf` on the floating-point
// operations, so the optimizer assumes no NaN or infinity: the checks on a
// runtime NaN and infinity fold to false at -O2, as clang's do, while a
// comparison with a finite value stays (WP-806).
//@ expect: run
//@ args: -ffinite-math-only -O2
//@ stdout: __FINITE_MATH_ONLY__ 1
//@ stdout: no __FAST_MATH__
//@ stdout: self-unequal 0
//@ stdout: isnan 0 isinf 0
//@ stdout: one < two 1
#include <cmath>
#include <cstdio>

volatile double vzero = 0.0, vone = 1.0, vtwo = 2.0;

int main() {
  double z = vzero, n = z / z, i = 1.0 / z;
  std::printf("__FINITE_MATH_ONLY__ %d\n", __FINITE_MATH_ONLY__);
#ifdef __FAST_MATH__
  std::printf("__FAST_MATH__\n");
#else
  std::printf("no __FAST_MATH__\n");
#endif
  std::printf("self-unequal %d\n", n != n);
  std::printf("isnan %d isinf %d\n", std::isnan(n) ? 1 : 0, std::isinf(i) ? 1 : 0);
  std::printf("one < two %d\n", vone < vtwo);
}
