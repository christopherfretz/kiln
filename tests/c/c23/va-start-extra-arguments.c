// C23 7.16.1.4: `va_start(ap, ...)` discards every argument after `ap`.
// kiln accepts them and warns -Wvarargs, at the first discarded argument,
// when they are not exactly the last parameter's identifier, as GCC 16.1
// does (its wording differs); clang 22 warns on a second argument but
// rejects a third. Found running GCC's C++ testsuite.
//@ requires: clang>=22
//
//@ skip(clang): clang 22 rejects more than two va_start arguments
//@ std: c23
//@ expect: run
//@ strict-diags
//@ stdout: 6
//@ diag: 26:16 warning "second argument to 'va_start' is not the last non-variadic parameter"
//@ diag: 28:16 warning "second argument to 'va_start' is not the last non-variadic parameter"
//@ diag: 30:16 warning "second argument to 'va_start' is not the last non-variadic parameter"
//@ diag: 32:16 warning "second argument to 'va_start' is not the last non-variadic parameter"
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

int printf(const char *, ...);
int main(void) { printf("%d\n", sum(3, 1, 2, 3)); }
