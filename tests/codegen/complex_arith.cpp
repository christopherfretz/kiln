// GNU _Complex arithmetic (clang CGExprComplex, CX_Full): add/sub, the
// __mul*c3/__div*c3 libcalls behind the NaN check, real-operand shortcuts,
// integer complex formulas, __real__/__imag__, comparisons, conversions,
// __builtin_complex and constant-initialized globals (WP-211).
using C = _Complex double;
using CF = _Complex float;
using CL = _Complex long double;
using CI = _Complex int;
C add(C a, C b) { return a + b; }
C mul(C a, C b) { return a * b; }
C div(C a, C b) { return a / b; }
CF mulf(CF a, CF b) { return a * b; }
CL divl(CL a, CL b) { return a / b; }
C mixed(C a, double d) { return a * d + d - a / d; }
C rdiv(double d, C a) { return d / a; }
CI imul(CI a, CI b) { return a * b + a / b; }
double re(C a) { return __real__ a + __imag__ a; }
bool eq(C a, C b) { return a == b; }
bool ne(C a, double b) { return a != b; }
C neg(C a) { return -a + ~a; }
C conv(CF f, int i) { C r = f; r += i; return r; }
void set(C& z) { __real__ z = 1; __imag__ z *= 2; }
bool truth(C z) { return z ? true : false; }
double toreal(C z) { return (double)z; }
CI fromf(C z) { return (CI)z; }
C bc(double a, double b) { return __builtin_complex(a, b); }
C il(double a) { C z = {a, 2}; return z; }
C g = __builtin_complex(1.0, 2.0);
CF gf = 3.0f;
