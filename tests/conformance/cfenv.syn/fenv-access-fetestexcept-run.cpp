// [cfenv.syn], C23 7.6.1: under `#pragma STDC FENV_ACCESS ON` the program
// may test the floating-point status flags: an inexact division raises
// FE_INEXACT, an exact one does not, after `feclearexcept` (constrained
// intrinsics, spec/codegen.md "Floating-point semantics", WP-1744).
//@ expect: run
//@ stdout: exact 0
//@ stdout: inexact 1
#include <cfenv>
#include <cstdio>

#pragma STDC FENV_ACCESS ON

volatile double one = 1.0, two = 2.0, three = 3.0;

int main() {
  std::feclearexcept(FE_ALL_EXCEPT);
  volatile double q = one / two;
  std::printf("exact %d\n", std::fetestexcept(FE_INEXACT) != 0);
  std::feclearexcept(FE_ALL_EXCEPT);
  q = one / three;
  std::printf("inexact %d\n", std::fetestexcept(FE_INEXACT) != 0);
  (void)q;
}
