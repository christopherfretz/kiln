// [expr.cond]/2.1: when one operand of `?:` is a throw-expression, the
// result is the other operand, also of `_Complex` type: the throwing arm
// leaves no value, and the result is the other arm's parts without a
// merge (WP-1739; GCC accepts it, clang 22 stops with "cannot compile
// this complex expression yet").
//@ expect: run
//@ oracle: gcc
//@ skip(clang): clang rejects a complex conditional with a throw arm
//@ stdout: 1 2 1 2 caught 3
#include <cstdio>
_Complex double mk() {
  _Complex double r = 1.0;
  __imag__ r = 2.0;
  return r;
}
_Complex double x = mk();
int main(int argc, char **) {
  _Complex double c = argc ? x : throw 1;
  _Complex double d = argc > 5 ? throw 2 : x;
  int e = 0;
  try {
    _Complex double f = argc > 5 ? x : throw 3;
    e = (int)__real__ f;
  } catch (int i) {
    e = i;
  }
  std::printf("%g %g %g %g %s %d\n", __real__ c, __imag__ c, __real__ d, __imag__ d,
              e == 3 ? "caught" : "no", e);
  return 0;
}
