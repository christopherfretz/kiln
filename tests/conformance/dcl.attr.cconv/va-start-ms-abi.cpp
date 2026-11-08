// [cstdarg.syn] [dcl.attr.grammar]/5 `va_start` in an `ms_abi` function
// on a Linux target is an error (clang `checkVAStartABI`: the Win64
// `va_list` is `__builtin_ms_va_list`); the target's own `va_list`
// works in a default-convention function. spec/types.md §5.4, WP-808.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:3 error "'va_start' used in Win64 ABI function"
#include <stdarg.h>
__attribute__((ms_abi)) int f(int n, ...) {
  va_list ap;
  va_start(ap, n);
  int r = va_arg(ap, int);
  va_end(ap);
  return r;
}
int g(int n, ...) {
  va_list ap;
  va_start(ap, n);
  int r = va_arg(ap, int);
  va_end(ap);
  return r;
}
