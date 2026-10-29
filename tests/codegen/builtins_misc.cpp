// [ptr.launder], [new.delete], [support.start.term] support: launder is the
// pointer, operator new/delete call the global replaceable functions with the
// builtin attribute, unreachable ends the block; [c.math] library calls.
using size_t = decltype(sizeof 0);
struct S { int x; };
S* a(S* p) { return __builtin_launder(p); }
void* b(size_t n) { return __builtin_operator_new(n); }
void c(void* p) { __builtin_operator_delete(p); }
int d(int x) { if (x) return 1; __builtin_unreachable(); }
double e(double x, int* i) { return __builtin_exp(x) + __builtin_exp2(x) + __builtin_expm1(x) + __builtin_frexp(x, i); }
float f(float x, int i, long l) { return __builtin_ldexpf(x, i) + __builtin_powf(x, x) + __builtin_scalbnf(x, i) + __builtin_scalblnf(x, l); }
long double g(long double x, int i) { return __builtin_expl(x) + __builtin_ldexpl(x, i) + __builtin_powl(x, x) + __builtin_frexpl(x, &i); }
int* h(int* p) { return __builtin_launder(p); }
int gv;
int d2(int x) { if (x) return 1; __builtin_unreachable(); }
void e2(int x) { gv = 2; __builtin_unreachable(); gv = 3; }
int f2(int x) { return x ? 1 : (__builtin_unreachable(), 2); }
void h2() { __builtin_unreachable(); }
int k2() { __builtin_unreachable(); }
int m2(int x) { switch (x) { case 1: return 3; default: __builtin_unreachable(); } }
void n2(int x) { while (x) { if (x == 3) __builtin_unreachable(); --x; } }
using T = decltype(sizeof 0);
void* q(size_t n) { void* x = __builtin_operator_new(n); __builtin_operator_delete(x); return __builtin_launder((S*)x); }
int r(double x, int* e) { return __builtin_frexpf((float)x, e) > 0; }
