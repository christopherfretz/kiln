// _Complex through varargs, lambdas, constructors, small integer element
// types, templates, arrays, struct parameters and casts (WP-211).
using C = _Complex double;
using CF = _Complex float;
using CI = _Complex int;
using CS = _Complex short;
using CC = _Complex char;
using CLL = _Complex long long;
void vf(int, ...);
void va(C c) { vf(1, c); }
auto lam(C c) { auto l = [c](C x) { return c + x; }; return l(c); }
struct B { B(C c) : v(c) {} C v; };
B mk(C c) { return B(c); }
CS sh(CS a, CS b) { return a * b; }
CC ch(CC a) { return a; }
CLL ll(CLL a, CLL b) { return a / b; }
template <class T> T tmul(T a, T b) { return a * b; }
C ti(C a) { return tmul(a, a); }
C cond2(bool b, C x, C y) { return b ? x : y; }
C arr[2] = {1.0, __builtin_complex(2.0, 3.0)};
C ga() { return arr[1]; }
struct P { CF a; CF b; };
P pp(P p) { return p; }
CF retf(CF a) { CF r; r = a; return r; }
void ptr(C* p) { p[1] = p[0]; __real__ *p = 2; }
double d2(C a) { double d = (double)a + (bool)a; return d; }
C ex(double d) { return (C)d; }
