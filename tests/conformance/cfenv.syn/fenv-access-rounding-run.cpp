// [cfenv.syn], C23 7.6.1: under `#pragma STDC FENV_ACCESS ON` the
// rounding mode `fesetround` sets is the one FP operations use, even when
// the optimizer could fold them: at -O2 `third(1.0)` rounded upward is
// greater than rounded downward (constrained intrinsics, WP-1744).
//@ expect: run
//@ args: -O2
//@ stdout: up>down 1
//@ stdout: nearest 1
#include <cfenv>
#include <cstdio>

#pragma STDC FENV_ACCESS ON

static double third(double x) { return x / 3.0; }

int main() {
  std::fesetround(FE_UPWARD);
  double up = third(1.0);
  std::fesetround(FE_DOWNWARD);
  double down = third(1.0);
  std::fesetround(FE_TONEAREST);
  std::printf("up>down %d\n", up > down);
  std::printf("nearest %d\n", third(1.0) == 1.0 / 3.0);
}
