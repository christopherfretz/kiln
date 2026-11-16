//@ expect: run
//@ strict-diags
//@ oracle: gcc
//@ xfail(clang): clang 22's <stdarg.h> has no C++26 one-argument va_start
//@ stdout: 6
//@ diag: 26:16 warning "second argument to 'va_start' is not the last non-variadic parameter"
//@ diag: 28:16 warning "second argument to 'va_start' is not the last non-variadic parameter"
//@ diag: 30:16 warning "second argument to 'va_start' is not the last non-variadic parameter"
//@ diag: 32:16 warning "second argument to 'va_start' is not the last non-variadic parameter"
// [cstdarg.syn] (C++26, C23 7.16.1.4): the arguments of `va_start` after
// the va_list are discarded. kiln accepts any tokens there and warns
// -Wvarargs, at the first of them, unless they are exactly the last named
// parameter's identifier, as GCC 16.1 does (its wording differs); a
// `va_start(ap, n)` spread over two lines is fine. Found running GCC's
// C++ testsuite.
//@ requires: clang>=22
#include <stdarg.h>

int sum(int n, ...) {
  va_list ap;
  int s = 0;
  va_start(ap);
  va_end(ap);
  va_start(ap, n);
  va_end(ap);
  va_start(ap, n, 1);
  va_end(ap);
  va_start(ap, 0);
  va_end(ap);
  va_start(ap, 1, 2, 3);
  va_end(ap);
  va_start(ap, + - garbage ]);
  va_end(ap);
  va_start(ap,
           n);
  for (int i = 0; i < n; ++i)
    s += va_arg(ap, int);
  va_end(ap);
  return s;
}

extern "C" int printf(const char *, ...);
int main() { printf("%d\n", sum(3, 1, 2, 3)); }
