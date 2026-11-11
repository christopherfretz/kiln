// C23 7.16.1.4: `va_start(ap, ...)` takes no parameter name; clang's
// <stdarg.h> expands it to `__builtin_va_start(ap, 0)` and clang
// `BuiltinVAStart` skips the last-named-parameter check for a second
// argument that is an integer constant expression of value 0 in C23.
// Before WP-1311 kiln warned -Wvarargs ("second argument to 'va_start'
// is not the last non-variadic parameter") on every C23 `va_start`,
// which broke -Werror builds.
//@ std: c23
//@ expect: run
//@ strict-diags
//@ args: -Werror
//@ stdout: 6 11 3
#include <stdarg.h>
#include <stdio.h>

static int sum(int n, ...) {
  va_list ap;
  va_start(ap, n);
  int s = 0;
  for (int i = 0; i < n; i++)
    s += va_arg(ap, int);
  va_end(ap);
  return s;
}

static int sum_unnamed(int n, ...) {
  va_list ap;
  va_start(ap);
  int s = 0;
  for (int i = 0; i < n; i++)
    s += va_arg(ap, int);
  va_end(ap);
  return s;
}

static int only_variadic(...) {
  va_list ap;
  va_start(ap);
  int x = va_arg(ap, int);
  va_end(ap);
  return x;
}

int main(void) {
  printf("%d %d %d\n", sum(3, 1, 2, 3), sum_unnamed(2, 5, 6), only_variadic(3));
  return 0;
}
