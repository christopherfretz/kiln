// -ffp-model=fast is -ffast-math under clang 19, and fast math that
// keeps NaNs and infinities under clang 22, whose promoted complex
// division kiln does not implement (an error unless a complex-range flag
// follows; spec/driver.md "Floating-point flags", WP-806).
//@ args: -ffp-model=fast -fcomplex-arithmetic=basic
extern "C" {
double sqrt(double);
float sqrtf(float);
double pow(double, double);
double fmod(double, double);
long double fmodl(long double, long double);
long lround(double);
double fabs(double);
int printf(const char *, ...);
}
typedef float v4f __attribute__((vector_size(16)));
double arith(double a, double b, double c) { return a * b + c / a - -b; }
bool cmp(float a, float b) { return a < b || a != b; }
float sel(bool c, float a, float b) { float r = c ? a : b; return r; }
float phi(bool c, float a, float b) { return (c && a > b) ? a : b; }
double lib(double x) { return sqrt(x) + pow(x, 2.0) + fmod(x, 3.0) + fabs(x) + lround(x); }
long double libl(long double x, int e) { return __builtin_ldexpl(x, e) + fmodl(x, x); }
float bi(float x) { return __builtin_sqrtf(x) + sqrtf(x) + (float)__builtin_llrintf(x); }
v4f vec(v4f a, v4f b) { return a * b + a; }
_Complex double cmul(_Complex double a, _Complex double b) { return a * b; }
_Complex float cdiv(_Complex float a, _Complex float b) { return a / b; }
double call(double x) { printf("%f\n", x); return arith(x, x, x); }
struct P { float x, y; };
P pass(P p, double d) { p.x += (float)d; return p; }
