// WP-186: a call of a C library function declared with its library
// signature lowers as clang `EmitBuiltinExpr` does at -O0 (clang 19 and
// 22, AArch64 and x86_64 alike): memcpy / memmove / memset become the
// llvm.mem* intrinsics, abs / labs / llabs become llvm.abs, and the
// always-const <math.h> functions with an elementwise intrinsic (fabs,
// ceil, floor, trunc, round, rint, nearbyint, copysign, fmax, fmin, fma
// and their f / l forms) become llvm.fabs ... llvm.fma. strlen, memcmp,
// sqrt (-fmath-errno), sin and pow stay ordinary library calls;
// [library.c], spec/codegen.md §3.
extern "C" {
typedef __SIZE_TYPE__ size_t;
void *memcpy(void *, const void *, size_t);
void *memset(void *, int, size_t);
void *memmove(void *, const void *, size_t);
size_t strlen(const char *);
int memcmp(const void *, const void *, size_t);
int abs(int);
long labs(long);
long long llabs(long long);
double fabs(double);
float fabsf(float);
long double fabsl(long double);
double ceil(double);
float floorf(float);
double trunc(double);
double round(double);
double rint(double);
double nearbyint(double);
double copysign(double, double);
double fmax(double, double);
float fminf(float, float);
double fma(double, double, double);
double sqrt(double);
double sin(double);
double pow(double, double);
}
void *mem(char *d, const char *s) {
  memcpy(d, s, 4);
  memset(d, 0, 4);
  return memmove(d, s, 4);
}
long ints(int i, long l, long long ll) { return abs(i) + labs(l) + llabs(ll); }
double fp(double x, float y, long double z) {
  return fabs(x) + fabsf(y) + (double)fabsl(z) + ceil(x) + floorf(y) + trunc(x) +
         round(x) + rint(x) + nearbyint(x) + copysign(x, x) + fmax(x, x) + fminf(y, y) +
         fma(x, x, x);
}
double calls(const char *s, double x) {
  return strlen(s) + memcmp(s, s, 2) + sqrt(x) + sin(x) + pow(x, x);
}
