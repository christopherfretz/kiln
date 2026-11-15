//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
// [cstdarg.syn]: __STDC_VERSION_STDARG_H__ comes with C++26 (P3348R4); a
// C++23 <stdarg.h> does not define it, and va_start still needs the
// parameter name there. Found running GCC's C++ testsuite.
#include <stdarg.h>
#ifdef __STDC_VERSION_STDARG_H__
#error defined before C++26
#endif
int f(int n, ...) {
  va_list ap;
  va_start(ap, n);
  int x = va_arg(ap, int);
  va_end(ap);
  return x;
}
