// [c.math.syn], [c.math.abs], [c.math.fpclass]: the libm builtins. abs is
// llvm.abs; fabs, copysign, fmax/fmin, ceil..nearbyint and fma are LLVM
// intrinsics; inf and constant arguments fold; fpclassify is a compare
// chain into a phi; the rest call the library (Const ones willreturn,
// memory(none)); expect is its value at -O0; abort is noreturn.
int a(int x) { return __builtin_abs(x); }
long b(long x) { return __builtin_labs(x); }
long long c(long long x) { return __builtin_llabs(x); }
int d(double x) { return __builtin_fpclassify(0, 1, 2, 3, 4, x); }
int d2(double x) { return __builtin_fpclassify(5, 6, 7, 8, 9, x * 2); }
int d3(float x) { return __builtin_fpclassify(0, 1, 2, 3, 4, x) + 1; }
int d4(long double x, int n) { return x > 0 ? __builtin_fpclassify(n, 1, 2, 3, 4, x) : n; }
int e(float x) { return __builtin_isinf_sign(x); }
int e2(long double x) { return __builtin_isinf_sign(x); }
int f(long double x) { return __builtin_issubnormal(x) + __builtin_iszero(x) + __builtin_issignaling(x); }
int f2(float x) { return __builtin_issubnormal(x) + __builtin_iszero(x) + __builtin_issignaling(x); }
double g() { return __builtin_inf(); }
float g2() { return __builtin_inff(); }
long double g3() { return __builtin_infl(); }
double h() { return __builtin_fabs(-2.0) + __builtin_fmax(1.0, 3.0); }
long i(long x) { return __builtin_expect(x, 1); }
void j(bool x) { __builtin_assume(x); }
void k() { __builtin_trap(); }
void l() { __builtin_abort(); }
double m(double x, double y) { return __builtin_copysign(x, y) + __builtin_fmin(x, y) + __builtin_fma(x, y, x) + __builtin_fmax(x, y); }
float m2(float x, float y) { return __builtin_copysignf(x, y) + __builtin_fminf(x, y) + __builtin_fmaf(x, y, x) + __builtin_fmaxf(x, y) + __builtin_fabsf(x); }
long double m3(long double x, long double y) { return __builtin_copysignl(x, y) + __builtin_fminl(x, y) + __builtin_fmal(x, y, x) + __builtin_fmaxl(x, y); }
long double n(long double x) { return __builtin_fabsl(x) + __builtin_ceill(x) + __builtin_sqrtl(x) + __builtin_cbrtl(x); }
double n2(double x) { return __builtin_floor(x) + __builtin_trunc(x) + __builtin_round(x) + __builtin_rint(x) + __builtin_nearbyint(x) + __builtin_ceil(x); }
float n3(float x) { return __builtin_floorf(x) + __builtin_truncf(x) + __builtin_roundf(x) + __builtin_rintf(x) + __builtin_nearbyintf(x) + __builtin_ceilf(x); }
long double n4(long double x) { return __builtin_floorl(x) + __builtin_truncl(x) + __builtin_roundl(x) + __builtin_rintl(x) + __builtin_nearbyintl(x); }
float o(float x, int *p) { return __builtin_modff(x, &x) + __builtin_frexpf(x, p) + __builtin_ldexpf(x, 3) + __builtin_fmodf(x, x) + __builtin_lroundf(x); }
double o2(double x, double y, int *q) { return __builtin_sqrt(x) + __builtin_sin(x) + __builtin_cos(y) + __builtin_atan2(x, y) + __builtin_hypot(x, y) + __builtin_remquo(x, y, q) + __builtin_log(x) + __builtin_nextafter(x, y); }
long double o3(long double x) { return __builtin_nexttowardl(x, x) + __builtin_tgammal(x) + __builtin_llrintl(x) + __builtin_ilogbl(x) + __builtin_logbl(x); }
float o4(float x) { return __builtin_cbrtf(x) + __builtin_erff(x) + __builtin_fdimf(x, x) + __builtin_lgammaf(x) + __builtin_remainderf(x, x); }
double p() { return __builtin_copysign(3.0, -0.0) + __builtin_fmin(-0.0, 0.0) + __builtin_fabs(-__builtin_inf()); }
int q() { return __builtin_fpclassify(0, 1, 2, 3, 4, 1e-310) + __builtin_isinf_sign(-__builtin_inff()); }
