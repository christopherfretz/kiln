/* The <tgmath.h> calls of tests/c/run/tgmath-<std>.c: each generic
   macro picks the float, double, long double or complex function by its
   arguments' types (C17 7.25), shown with _Generic and the values. */
#include <tgmath.h>
#include <stdio.h>
#define T(x) _Generic((x), float: "f", double: "d", long double: "ld", \
  float _Complex: "cf", double _Complex: "cd", long double _Complex: "cld", default: "?")
static int tgmath_calls(void) {
  float f = 4.0f;
  double d = 9.0;
  long double ld = 16.0L;
  double _Complex z = 3.0 + 4.0 * _Complex_I;
  float _Complex zf = 1.0f - 2.0f * _Complex_I;
  printf("%s %s %s %s\n", T(sqrt(f)), T(sqrt(d)), T(sqrt(ld)), T(sqrt(z)));
  printf("%s %s %s %s\n", T(pow(f, f)), T(pow(f, d)), T(pow(ld, 2)), T(pow(z, 2.0)));
  printf("%s %s %s %s\n", T(fabs(f)), T(fabs(-d)), T(fabs(ld)), T(fabs(zf)));
  printf("%s %s %s\n", T(creal(zf)), T(creal(z)), T(creal(d)));
  printf("%g %g %Lg %g\n", (double)sqrt(f), sqrt(d), sqrtl(ld), creal(sqrt(z)));
  printf("%g %g %g %g\n", (double)pow(f, 2), fabs(-d), (double)fabs(zf), creal(z));
  return 0;
}
