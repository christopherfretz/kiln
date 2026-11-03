// [expr.pre]/4, [basic.start.static]: floating arithmetic of a global's
// initializer that yields a NaN or divides by zero is not a constant
// expression, but clang's ConstantEmitter folds it (its folding modes go
// on past that undefined behaviour) and emits the APFloat value: the first
// NaN operand, quieted, sign and payload kept (no flip for '-' or '*'),
// the default positive quiet NaN for an invalid operation, the payload
// shifted by the difference in precision on a conversion. No dynamic
// initialization, on both triples and every format.
constexpr double q = __builtin_bit_cast(double, 0x7ff8000000000005ull);
constexpr double s = __builtin_bit_cast(double, 0xfff0000000000007ull);
constexpr double inf = __builtin_inf();
double r1 = q + s;
double r2 = s + q;
double r3 = inf - inf;
double r4 = 0.0 * inf;
double r5 = 0.0 / 0.0;
double r6 = s * 2.0;
double r7 = 3.0 - s;
double r8 = 1.0 / 0.0;
double r9 = -1.0 / -0.0;
_Float16 h1 = (_Float16)s + (_Float16)1;
_Float16 h2 = (_Float16)inf - (_Float16)inf;
float f1 = (float)s;
float f2 = (float)q - 1.0f;
long double l1 = (long double)s / 2;
long double l2 = 2.0L - (long double)q;
__bf16 b1 = (__bf16)q * (__bf16)2;
constexpr double twice(double x) { return x + x; }
double c1 = twice(q);
