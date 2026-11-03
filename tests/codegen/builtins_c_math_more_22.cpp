//@ min-oracle: 22
// [c.math] (WP-419): clang 22's __builtin_fmaximum_numf128 /
// __builtin_fminimum_numf128 lower to llvm.maximumnum / llvm.minimumnum
// of fp128 and fold when constant; __builtin_sincosf128 and
// __builtin_sincospi / sincospif / sincospil are nounwind library calls of
// the function they name (AArch64 and x86_64 alike; on AArch64 the f128
// forms take long double, which is fp128). clang 19 declares none of them.
#if __has_builtin(__builtin_sincospi) && __has_builtin(__builtin_fmaximum_numf128)
using Q = decltype(__builtin_inff128());
Q q, r, s;
void a() {
  r = __builtin_fmaximum_numf128(q, r);
  s = __builtin_fminimum_numf128(q, r);
  __builtin_sincosf128(q, &r, &s);
}
Q b() { return __builtin_fmaximum_numf128(Q(1), Q(2)) + __builtin_fminimum_numf128(Q(1), Q(2)); }
void c(double x, double *p, double *q) { __builtin_sincospi(x, p, q); }
void d(float x, float *p, float *q) { __builtin_sincospif(x, p, q); }
void e(long double x, long double *p, long double *q) { __builtin_sincospil(x, p, q); }
bool f(double x, double *p, double *q) { return noexcept(__builtin_sincospi(x, p, q)); }
#endif
