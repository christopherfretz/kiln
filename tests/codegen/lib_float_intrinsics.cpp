// [c.math], clang EmitBuiltinExpr (WP-419): a C library math function
// declared with C language linkage is clang's library builtin, and a
// direct call of an errno-free one lowers as its __builtin_ form, the LLVM
// intrinsic (ceil -> llvm.ceil; long double picks the intrinsic overload);
// sqrt / sin set errno under the default -fmath-errno and stay library
// calls. C23's roundeven is recognized by clang 19 and 22; fmaximum_num /
// fminimum_num only by clang 22 (clang 19 calls the library function).
// sincos is a GNU library builtin only (an ordinary call in C++26). A
// declaration with C++ linkage, a static function and a call through a
// pointer are ordinary calls; a definition stays the builtin.
extern "C" {
double ceil(double); float ceilf(float); long double ceill(long double);
double floor(double); float floorf(float); double trunc(double); double round(double);
double rint(double); double nearbyint(double); double sqrt(double); double sin(double);
double fabs(double); long double fabsl(long double); double copysign(double, double);
double fma(double, double, double); double fmax(double, double); float fminf(float, float);
double roundeven(double); float roundevenf(float); long double roundevenl(long double);
double fmaximum_num(double, double); float fminimum_numf(float, float);
void sincos(double, double *, double *);
}
namespace std { extern "C" double round(double); }
extern "C" double trunc(double x);
double a(double x) { return ceil(x) + floor(x) + trunc(x) + round(x) + rint(x) + nearbyint(x); }
float b(float x, float y) { return ceilf(x) + floorf(x) + fminf(x, y) + roundevenf(x); }
long double c(long double x) { return ceill(x) + fabsl(x) + roundevenl(x); }
double d(double x, double y) { return fabs(x) + copysign(x, y) + fma(x, y, x) + fmax(x, y); }
double e(double x) { return sqrt(x) + sin(x) + roundeven(x) + std::round(x); }
double f(double x, double y) { return fmaximum_num(x, y) + fminimum_numf(x, y); }
void g(double x, double *s, double *c) { sincos(x, s, c); }
double ceil(double, int);
static double rint(double x, double) { return x; }
double h(double x) { return ceil(x, 1) + rint(x, x); }
double (*p)(double) = nearbyint;
double k(double x) { return p(x) + ceil(2.5); }
