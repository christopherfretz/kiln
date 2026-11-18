// [expr.cond]/5, [expr.const] (C23 6.7.2.1): a glvalue conditional whose
// operands are bit-fields of a `_BitInt(N)` with N > 128 (or one such
// bit-field and an object of that type) is evaluated in a constant
// expression: a read, `=`, `+=` (also with a floating computation type),
// `*=`, `++` and `--`, and a nested conditional, each reach the operand
// the condition selected. clang is the oracle; x86-64 only (aarch64's
// `__BITINT_MAXWIDTH__` is 128).
//@ args: --target=x86_64-unknown-linux-gnu
//@ strict-diags
using W = _BitInt(200);
struct S {
  W a : 150;
  W b : 180;
};
constexpr long long f(bool c, bool d) {
  S s{1, 2};
  W v = 7;
  (c ? s.a : s.b) = 5;
  (c ? s.a : s.b) += 3;
  ++(c ? s.a : s.b);
  (d ? s.a : v) *= 2;
  (d ? (c ? s.a : s.b) : v)--;
  (c ? s.a : s.b) += 1.0;
  W r = c ? s.a : s.b;
  return (long long)r * 10000 + (long long)(c ? s.b : s.a) * 100 + (long long)v;
}
static_assert(f(true, true) == 180207);
static_assert(f(false, true) == 90207);
static_assert(f(true, false) == 100213);
static_assert(f(false, false) == 100113);
// A store truncates to the selected bit-field's width.
constexpr bool g(bool c) {
  S s{0, 0};
  (c ? s.a : s.b) = W(1) << 160;
  return c ? s.a == 0 : s.b == W(1) << 160;
}
static_assert(g(true) && g(false));
