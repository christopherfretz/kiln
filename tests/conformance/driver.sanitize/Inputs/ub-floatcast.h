// Conversions for the float-cast-overflow check, _BitInt and _Atomic
// destinations and complex parts included (clang EmitFloatConversionCheck;
// WP-871).
namespace n1 {
enum E { A, B = 100 };
enum class F : unsigned char { X };
int f0(float f) { return f; }
unsigned f1(double d) { return d; }
long long f2(double d) { return (long long)d; }
unsigned long long f3(float f) { return static_cast<unsigned long long>(f); }
short f4(long double d) { return d; }
__int128 f5(double d) { return d; }
unsigned __int128 f6(float f) { return f; }
int f7(__fp16 *h) { return *h; }
char f8(_Float16 h) { return h; }
bool f9(double d) { return d; }
E f10(double d) { return (E)d; }
F f11(float d) { return (F)d; }
void f12(int &i, double d) { i += d; i *= 1.5f; }
int f13(__bf16 b) { return b; }
long long f14(long double d) { return d; }
signed char f15(long double d) { return d; }
_BitInt(37) f16(double d) { return d; }
unsigned _BitInt(100) f17(long double d) { return d; }
_BitInt(128) f18(float d) { return d; }
double f19(int i) { return i; }
float f20(double d) { return d; }
} // namespace n1
namespace n2 {
_Complex double cd; _Complex int ci;
void f(double d, int &i, _Complex float cf) { ci = cd; ci = cf; ci += d; i = (int)cd; cd = ci; }
} // namespace n2
namespace n3 {
_Atomic(int) i; _Atomic(unsigned) u; _Atomic(float) f; _Atomic(short) s;
void g(double d, long l, int v) { i += d; u -= d; i *= d; s += d; f += l; f++; s <<= v; u >>= v; i %= v; }
_Complex double cd;
void h(double d) { i = (int)cd; int k = (int)cd; (void)k; }
} // namespace n3
