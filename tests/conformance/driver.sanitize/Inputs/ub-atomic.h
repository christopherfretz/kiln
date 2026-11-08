// _Atomic ++/-- and compound assignment under the overflow checks (clang
// EmitScalarPrePostIncDec, EmitCompoundAssignLValue; WP-871).
namespace n1 {
_Atomic(unsigned) u; _Atomic(int) i; _Atomic(unsigned char) uc; _Atomic(short) s; _Atomic(unsigned long) ul;
void f1() { u++; --u; i++; --i; uc++; s--; ul++; }
void f2(int v, unsigned w, long l) { u += w; u -= w; u *= w; i += v; i *= v; uc += v; s -= v; s *= v; ul += l; i <<= v; i /= v; u /= w; }
unsigned f3() { return ++u; }
int f4() { return i++; }
} // namespace n1
namespace n2 {
_Atomic(int) i; _Atomic(unsigned) u; _Atomic(short) s;
void g(int v) { s <<= v; u >>= v; i %= v; u -= v; s += v; }
} // namespace n2
