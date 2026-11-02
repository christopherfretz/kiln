// GNU builtins libstdc++ calls directly: __builtin_mul_overflow (usable in
// constant expressions; bits/stl_bvector.h, bits/max_size_type.h, 49 headers),
// __builtin_alloca and __builtin_vsnprintf (bits/basic_string.h __to_xstring,
// 36 headers) (docs/status/libstdcxx.md, WP-188).
// Verified against clang 19.
//@ expect: compile-ok
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
constexpr bool ovf() { int r = 0; return __builtin_mul_overflow(1 << 30, 4, &r); }
static_assert(ovf());
