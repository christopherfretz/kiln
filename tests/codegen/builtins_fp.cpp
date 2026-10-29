// [c.math.fpclass], [c.math.syn]: classification (llvm.is.fpclass, the sign
// bit, the quiet comparisons) and the huge_val / nan constants, which fold;
// nan of a non-literal string calls the library.
int a(double x) { return __builtin_isfinite(x) + __builtin_isinf(x) + __builtin_isnan(x); }
int b(float x) { return __builtin_isnormal(x) + __builtin_signbit(x); }
int b2(long double x) { return __builtin_isnan(x) + __builtin_signbit(x) + __builtin_isinf(x); }
int c(double x, double y) { return __builtin_isgreater(x, y) + __builtin_isgreaterequal(x, y) + __builtin_isless(x, y); }
int d(float x, double y) { return __builtin_islessequal(x, y) + __builtin_islessgreater(x, y) + __builtin_isunordered(x, y); }
double e() { return __builtin_huge_val() + __builtin_nan("") + __builtin_nans("1"); }
float f() { return __builtin_huge_valf() + __builtin_nanf("0x10"); }
long double g() { return __builtin_nansl(""); }
long double g2() { return __builtin_huge_vall(); }
double g3() { return __builtin_huge_val() - __builtin_huge_val(); }
double h(const char* s) { return __builtin_nan(s); }
double d2(const char* s) { return __builtin_nan(s) + __builtin_nans(s); }
float e2(const char* s) { return __builtin_nanf(s); }
long double f2(const char* s) { return __builtin_nanl(s); }
int g4(double x) { if (__builtin_isnan(x)) return 1; return __builtin_isinf(x) ? 2 : 3; }
int h2(float x) { return __builtin_isgreater(x, 1.0f); }
long double i2(long double x) { return __builtin_huge_vall() * x; }
int j2(unsigned long x) { return __builtin_clzg(x, -1) + __builtin_ctzg((unsigned char)x, 9); }
bool k(int x) { return __builtin_popcountg((unsigned)x) == 1; }
int l(long double x) { return __builtin_isfinite(x) + __builtin_isnormal(x) + __builtin_isunordered(x, 1.0L); }
int m(int x, double y) { return __builtin_isless(x, y); }
