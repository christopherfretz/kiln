// [conv.fpint]/1-2, [expr.ass]/7 (C23 6.3.1.4): constant evaluation of the
// conversions between a `_BitInt(N)` with N > 128 and a floating type. An
// integer converts to the nearest value (ties to even; past the type's range
// to infinity); a floating value truncates toward zero. A compound
// assignment whose computation type is floating converts the wide operand
// both ways. clang is the oracle; x86-64 only (aarch64's
// `__BITINT_MAXWIDTH__` is 128).
//@ args: --target=x86_64-unknown-linux-gnu
using W = _BitInt(200);
using U = unsigned _BitInt(256);
using X = _BitInt(129);
// Integral to floating.
constexpr double tod(W v) { return (double)v; }
constexpr float tof(U v) { return v; }
constexpr long double told(W v) { return v; }
static_assert(tod(W(1) << 150) == 0x1p150);
static_assert(tod(-(W(1) << 100) - 1) == -0x1p100);
static_assert(tod((W(1) << 160) + (W(1) << 107)) == 0x1p160);               // tie, even
static_assert(tod((W(1) << 160) + (W(1) << 107) + 1) == 0x1p160 + 0x1p108); // sticky
static_assert(tod((W(1) << 160) + (W(3) << 107)) == 0x1p160 + 0x1p109);     // tie, up
static_assert(tod(0) == 0 && tod(-7) == -7.0);
static_assert(tof(~U(0)) == __builtin_inff()); // past FLT_MAX
static_assert(tof(U(1) << 127) == 0x1p127f);
static_assert(told((W(1) << 130) + 1) == 0x1p130L);
static_assert((double)(X)-1 == -1.0);
static_assert((double)((unsigned _BitInt(129))1 << 128) == 0x1p128);
// Floating to integral.
constexpr W fromd(double d) { return (W)d; }
constexpr U fromf(float f) { return f; }
static_assert(fromd(0x1p150) == W(1) << 150);
static_assert(fromd(-12345.9) == -12345);
static_assert(fromd(-0.5) == 0 && fromd(0.99) == 0);
static_assert(fromd(-0x1p199) == -(W(1) << 198) * 2);
static_assert(fromd(0x1.fffffffffffffp198) == (W(0x1fffffffffffff) << 146));
static_assert(fromf(3.5f) == 3 && fromf(0x1p127f) == U(1) << 127);
static_assert((X)0x1p127 == X(1) << 127);
static_assert((X)-0x1p128 == -(X(1) << 127) * 2);
// Compound assignment with a floating computation type.
constexpr W ca1() {
  W w = 10;
  w *= 2.5;
  w -= 0.5; // 24.5, truncated
  return w;
}
static_assert(ca1() == 24);
constexpr double ca2() {
  double d = 1.5;
  d += W(1) << 140;
  return d;
}
static_assert(ca2() == 0x1p140);
