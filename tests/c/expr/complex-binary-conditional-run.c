// GNU `x ?: y` with complex operands (WP-1739): the first operand is
// evaluated once, compared with zero as a complex number, and is the
// result when it is nonzero (clang `VisitAbstractConditionalOperator`
// binds it as an opaque value). An lvalue, a call, a compound literal and
// an integer complex operand; and an ordinary `?:` with a call arm.
//@ expect: run
//@ stdout: 1 2 1 2 3 4 5 0 2 2
#include <stdio.h>
_Complex double g(void) { static int n; n++; return n + 2.0i; }
_Complex double x = 1.0 + 2.0i, y = 3.0 + 4.0i;
_Complex int ci = 5;
int main(void) {
  _Complex double a = x ?: y;
  _Complex double b = g() ?: y;
  _Complex double c = (_Complex double){0} ?: y;
  _Complex int d = ci ?: (_Complex int)7;
  _Complex double e = 0 ? x : g();
  printf("%g %g %g %g %g %g %d %d %g %g\n", __real__ a, __imag__ a, __real__ b, __imag__ b,
         __real__ c, __imag__ c, __real__ d, __imag__ d, __real__ e, __imag__ e);
  return 0;
}
