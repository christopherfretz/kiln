// long double: x86_fp80 (MEMORY when passed on x86-64, returned in x87),
// fp128 on AArch64.
long double ld_add(long double a, long double b) { return a + b; }
long double ld_mix(double d, long double l, float f) { return d * l - f; }
struct LDS { long double v; };
LDS ld_ext(long double);
long double ld_unwrap(LDS s) { return s.v; }
LDS ld_pass(long double v) { return ld_ext(v); }
int ld_cmp(long double a, long double b) { return a < b ? -1 : a > b; }
long double ld_use(long double x) {
  return ld_unwrap(ld_ext(ld_add(x, 1.5L))) + ld_mix(2.0, x, 3.0f) + ld_unwrap(ld_pass(x)) + ld_cmp(x, 2);
}
