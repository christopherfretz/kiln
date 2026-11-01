// GNU builtins libstdc++ calls directly: __builtin_mul_overflow (usable in
// constant expressions; bits/stl_bvector.h, bits/max_size_type.h, 49 headers),
// __builtin_alloca and __builtin_vsnprintf (bits/basic_string.h __to_xstring,
// 36 headers) and the _Complex math builtins __builtin_cabs/__builtin_csqrtf
// and friends (<complex>, <ccomplex>, <ctgmath>) (docs/status/libstdcxx.md, WP-188).
// Verified against clang 19.
//@ expect: compile-ok
//@ xfail(kiln): use of undeclared identifier '__builtin_mul_overflow' (GNU builtins not provided)
#include <stdarg.h>
unsigned long mul(unsigned long a, unsigned long b) {
  unsigned long r;
  if (__builtin_mul_overflow(a, b, &r)) return 0;
  return r;
}
void *stack(unsigned long n) { return __builtin_alloca(n); }
int fmt(char *buf, unsigned long n, const char *f, va_list ap) {
  return __builtin_vsnprintf(buf, n, f, ap);
}
double mag(_Complex double z) { return __builtin_cabs(z); }
_Complex float rt(_Complex float z) { return __builtin_csqrtf(z); }
constexpr bool ovf() { int r = 0; return __builtin_mul_overflow(1 << 30, 4, &r); }
static_assert(ovf());
