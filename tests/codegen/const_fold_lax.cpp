// WP-165: clang's `tryEmitAsConstant` folds the read of a `const`
// non-volatile variable with `EvaluateAsRValue`, which also reads one that
// is not usable in constant expressions: `const double`, `const float`,
// `const long double`, `const _Atomic(T)`, a `const` class or array, a
// `const` pointer. A variable whose every use folds is not emitted. The
// read of a local (`return x;` is an xvalue no-op cast) folds too; a
// runtime initializer or the address of a local does not.
struct S { int a; double b; };
const double cd = 2.5;
const _Atomic(int) ca = 7;
const float cf = 1.5f;
const long double cld = 1.0L;
const S s = {1, 2.0};
const double arr[3] = {1.0, 2.0, 3.0};
const char *const str = "abc";
const double *const pcd = &cd;
const _Atomic(bool) gb = true;
const _Atomic(char) gc = 'a';
const _Atomic(double) gd = 1.5;
int gi;
int *const pgi = &gi;
double rt();

double f1() { return cd; }
int f2() { return ca; }
double f3() { const double l = 3.5; return l * 2; }
int f4() { const _Atomic(int) l = 4; return l + 1; }
float f5() { return cf; }
long double f6() { return cld; }
double f7() { static const double st = 9.0; return st; }
const double *f8() { return &cd; }
double f9() { return s.b; }
double f10() { return arr[1]; }
const char *f11() { return str; }
double f12() { return *pcd; }
double f13() { const double x = rt(); return x; }
int f14() { if (cd > 1.0) return 1; return 2; }
int *f15() { return pgi; }
int f16() { int y = 3; int *const p = &y; return *p; }
double f17(int i) { const double t = i; return t; }
bool f18() { const _Atomic(bool) b = true; return b; }
long f19() { const _Atomic(long) v = 5; return __c11_atomic_load(&v, 5); }
double f20() { return cd ? 1.0 : 2.0; }
bool f21() { return gb; }
int f22() { return gb ? 3 : 4; }
char f23() { return gc; }
double f24() { return gd; }
int f25() { const int l = 4; return l; }
int f26() { const _Atomic(char) c = 2; return c; }
