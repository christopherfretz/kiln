// [cfenv.syn], clang `-frounding-math`: without any pragma, FP operations
// use the rounding mode `fesetround` sets, even when the optimizer could
// fold them: at -O2 `third(1.0)` rounded upward is greater than rounded
// downward (constrained intrinsics with `round.dynamic`, WP-1798). A
// constant initializer is still evaluated in the default mode.
//@ expect: run
//@ args: -O2 -frounding-math
//@ stdout: up>down 1
//@ stdout: constant 1
#include <cfenv>
#include <cstdio>

static double third(double x) { return x / 3.0; }
constexpr double k = 1.0 / 3.0;

int main() {
  std::fesetround(FE_UPWARD);
  double up = third(1.0);
  std::fesetround(FE_DOWNWARD);
  double down = third(1.0);
  std::fesetround(FE_TONEAREST);
  std::printf("up>down %d\n", up > down);
  std::printf("constant %d\n", k == third(1.0));
  return 0;
}
