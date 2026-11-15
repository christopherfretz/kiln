//@ expect: run
//@ strict-diags
//@ oracle: gcc
//@ xfail(clang): clang 22's <stdarg.h> has no C++26 one-argument va_start
//@ stdout: 10 45 3 202311
// [cstdarg.syn] (C++26, P3348R4: C++26 refers to C23): `va_start(ap, ...)`
// takes the va_list alone; arguments after it are neither expanded nor
// evaluated (C23 7.16.1.4), so they need not name the last parameter, and
// a function whose parameter list is just `...` can walk its arguments.
// <stdarg.h> defines __STDC_VERSION_STDARG_H__ as 202311L (C23 7.16/1).
// No -Wvarargs warning (on by default): the `0` kiln's header passes is
// not the user's. GCC also warns about the ignored tokens; kiln does not.
// Found running GCC's C++ testsuite.
#include <stdarg.h>
extern "C" int printf(const char *, ...);

double only_ellipsis(...) {
  va_list ap;
  va_start(ap);
  double r = va_arg(ap, int);
  r += va_arg(ap, double);
  r += va_arg(ap, int);
  r += va_arg(ap, double);
  va_end(ap);
  return r;
}

int ignored_tail(int n, ...) {
  va_list ap;
  va_start(ap, these tokens are ! never * expanded);
  int s = 0;
  for (int i = 0; i < n; i++)
    s += va_arg(ap, int);
  va_end(ap);
  return s;
}

int promoted_named(char c, ...) {
  va_list ap;
  va_start(ap);
  int x = va_arg(ap, int);
  va_end(ap);
  return x + c;
}

int main() {
  printf("%g %d %d %ld\n", only_ellipsis(1, 2.0, 3, 4.0),
         ignored_tail(10, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9), promoted_named(1, 2),
         (long)__STDC_VERSION_STDARG_H__);
}
