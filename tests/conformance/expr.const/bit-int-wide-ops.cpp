// [expr.add], [expr.mul], [expr.shift], [expr.bit.and], [expr.rel],
// [conv.integral], [conv.bool] (C23 6.2.5, 6.3.1.3): constant evaluation of
// `_BitInt(N)` with N > 128, a memory-resident value of N bits in 64-bit limbs
// (spec/oven.md "`_BitInt`"): arithmetic exact to N bits (unsigned forms wrap),
// conversions to and from narrower and wider integers and `bool`,
// comparisons, shifts, bitwise operations, compound assignment, `++`/`--`,
// arrays, members, parameters and results. clang is the oracle; x86-64 only
// (aarch64's `__BITINT_MAXWIDTH__` is 128).
//@ args: --target=x86_64-unknown-linux-gnu
using W = _BitInt(200);
using U = unsigned _BitInt(200);
using V = unsigned _BitInt(129);
constexpr W one = 1;
constexpr W big = one << 150;
static_assert(big > 0 && (big >> 150) == 1);
static_assert(big * 4 / 4 == big && big % 7 == (one << 150) % 7);
static_assert(-big < 0 && -(-big) == big);
static_assert((big | 5) - big == 5 && (big & 5) == 0 && ((big ^ big) == 0));
static_assert(~W(0) == -1);
static_assert(U(-1) == (U(1) << 199) * 2 - 1);
static_assert(U(-1) + 1 == 0);
static_assert(V(-1) >> 128 == 1);
static_assert((long long)(big + 42) == 42);
static_assert((int)(W(-3)) == -3);
static_assert((unsigned _BitInt(64))(U(-1)) == ~0ull);
static_assert((_BitInt(300))(W(-5)) == -5);
static_assert((U)(W(-1)) == U(-1));
static_assert(W(1000000007) * 1000000007 % 998244353 == 740650005);
static_assert(!W(0) && W(3) && (W(3) ? true : false));
constexpr U fact(int n) { U r = 1; for (int i = 2; i <= n; ++i) r *= i; return r; }
static_assert(fact(40) / fact(38) == 40 * 39);
static_assert(fact(30) % 1000000007 == 109361473);
constexpr W f(W x) { W y = x; y += 3; y <<= 2; y -= 1; y++; --y; return y; }
static_assert(f(2) == 19);
constexpr W g() { W a[3] = {1, 2, 3}; W s = 0; for (W v : a) s += v * v; return s; }
static_assert(g() == 14);
struct S { W w; V v; };
constexpr S s{big, V(7)};
static_assert(s.w == big && s.v == 7);
static_assert(W(-7) / 2 == -3 && W(-7) % 2 == -1);
static_assert((W(-8) >> 1) == -4);
static_assert(W(5) <= W(5) && W(-1) < W(0) && U(-1) > U(0) && W(2) != W(3));
static_assert(sizeof(W) == 32);
using U4 = unsigned _BitInt(256);
using S4 = _BitInt(256);
constexpr U4 top = U4(1) << 255;
static_assert(top > (top >> 1) && top / 2 == top >> 1 && top * 2 == 0);
static_assert((S4)top < 0 && (S4)(top - 1) > 0);
static_assert(S4(-1) / S4(3) == 0 && S4(-9) / S4(-3) == 3);
constexpr U4 pw(U4 b, int e) { U4 r = 1; while (e--) r *= b; return r; }
static_assert(pw(10, 70) / pw(10, 69) == 10 && pw(2, 255) == top);
static_assert(pw(3, 160) % 1000000007 == 661385780);
constexpr U4 pick(bool c, U4 a, U4 b) { return c ? a : b; }
static_assert(pick(true, top, 0) == top && pick(false, top, 7) == 7);
constexpr S4 neg(S4 x) { return -x; }
static_assert(neg(neg(S4(top - 1))) == S4(top - 1));
constexpr int bits(U4 x) { int n = 0; while (x) { n += (int)(x & 1); x >>= 1; } return n; }
static_assert(bits(top - 1) == 255 && bits(U4(-1)) == 256);
constexpr int k = 10;
static_assert((U4(1) << k) == 1024 && (U4(1024) >> U4(10)) == 1);
