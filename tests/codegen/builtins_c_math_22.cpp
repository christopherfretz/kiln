//@ min-oracle: 22
// WP-410: clang 22's fmaximum_num / fminimum_num and sincos builtins for
// double, float and long double (-O0, AArch64 and x86_64 alike):
// fmaximum_num / fminimum_num lower to llvm.maximumnum / llvm.minimumnum
// of the operand type and fold when constant (clang 22 constant-evaluates
// them); sincos is a library call of sincos / sincosf / sincosl. clang 19
// declares none of them (the guard keeps the case valid there).
#if __has_builtin(__builtin_fmaximum_num) && __has_builtin(__builtin_sincos)
double d(double x, double y) {
  return __builtin_fmaximum_num(x, y) + __builtin_fminimum_num(x, y);
}
float f(float x, float y) {
  return __builtin_fmaximum_numf(x, y) + __builtin_fminimum_numf(x, y);
}
long double l(long double x, long double y) {
  return __builtin_fmaximum_numl(x, y) + __builtin_fminimum_numl(x, y);
}
double k() {
  return __builtin_fmaximum_num(1.0, 2.0) + __builtin_fminimum_numf(__builtin_nanf(""), 3.0f) +
         __builtin_fmaximum_numl(-0.0L, 0.0L);
}
void s(double x, double *sn, double *cs) { __builtin_sincos(x, sn, cs); }
void sf(float x, float *sn, float *cs) { __builtin_sincosf(x, sn, cs); }
void sl(long double x, long double *sn, long double *cs) { __builtin_sincosl(x, sn, cs); }
#endif
