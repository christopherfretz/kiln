//@ args: -std=gnu++26
// clang EmitBuiltinExpr (WP-419): in the GNU dialects clang 22 recognizes
// sincos / sincosf / sincosl as library builtins (NoThrow: the call is
// nounwind) and still calls the library function; clang 19 does not
// recognize them (an ordinary call). A definition of an errno-free math
// function is still the builtin.
extern "C" {
void sincos(double, double *, double *);
void sincosf(float, float *, float *);
void sincosl(long double, long double *, long double *);
double floor(double x) { return x; }
}
void f(double x, double *s, double *c) { sincos(x, s, c); }
void g(float x, float *s, float *c) { sincosf(x, s, c); }
void h(long double x, long double *s, long double *c) { sincosl(x, s, c); }
double k(double x) { return floor(x); }
