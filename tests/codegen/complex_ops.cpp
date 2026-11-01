// _Complex ++/--, volatile access, boolean contexts, long double and
// unsigned arithmetic, constexpr/static/global locals, virtual calls and
// list initialization (WP-211).
using C = _Complex double;
using CF = _Complex float;
using CL = _Complex long double;
using CI = _Complex int;
using CU = _Complex unsigned;
void inc(C& a, CI& b) { ++a; a++; --b; b--; C t = a++; (void)t; }
double sc(double d, C c) { d = __real__ c; return d; }
void vol(volatile C& v, C x) { v = x; C y = v; v += x; (void)y; }
bool tests(C a, CI b) { if (a) return !b; return a && b; }
C plus(C a) { return +a; }
double ri(double d, int i) { return __real__ d + __imag__ d + __imag__ i; }
CL lmul(CL a, CL b) { return a * b; }
CL ldiv2(CL a, double b) { return a / b; }
CU udiv(CU a, CU b) { return a / b; }
C ce() { constexpr C z = __builtin_complex(1.0, 2.0); return z; }
C st() { static C s = 3.0; return s += 1; }
C gl = 5.0;
C& ref() { return gl; }
C deref() { return ref() + ref(); }
struct M { C v; C get() const { return v; } virtual C vf(C x) { return x * v; } };
C callm(M& m) { return m.get() + m.vf(m.v); }
C comma(C a, C b) { return (a, b); }
CF cf(CF a, float f, int i) { return a * f + (CF)i - f / a; }
void cmpf(CF a, CI b, bool& r) { r = a == 1.0f; r = b != 2; r = 3 == b; }
C ilist() { C z{}; C y = {}; C w = {1.0}; return z + y + w; }
CI icomp(CI a, int i) { a *= i; a /= 2; a += a; return -a; }
