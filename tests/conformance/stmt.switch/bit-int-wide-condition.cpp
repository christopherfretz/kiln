// [stmt.switch]/2, [expr.const]: a switch whose condition is a `_BitInt(N)`
// with N > 128, or an `unsigned __int128` case value above the `__int128`
// maximum, is evaluated by its exact value in a constant expression (each
// case converted to the condition's type, compared limb by limb). clang is
// the oracle; x86-64 only (aarch64's `__BITINT_MAXWIDTH__` is 128).
//@ args: --target=x86_64-unknown-linux-gnu
using W = _BitInt(200);
using U = unsigned _BitInt(256);
constexpr int sw(W v) {
  switch (v) {
  case 1: return 10;
  case -1: return 20;
  case W(1) << 160: return 40;
  case -(W(1) << 190): return 50;
  case (unsigned _BitInt(140))1 << 139: return 60;
  default: return 30;
  }
}
static_assert(sw(1) == 10 && sw(-1) == 20 && sw(5) == 30);
static_assert(sw(W(1) << 160) == 40 && sw(-(W(1) << 190)) == 50);
static_assert(sw(W(1) << 139) == 60 && sw((W(1) << 160) + 1) == 30);
constexpr int sw2(U v) {
  switch (v) {
  case 0: return 1;
  case ~U(0): return 2;
  }
  return 3;
}
static_assert(sw2(0) == 1 && sw2(~U(0)) == 2 && sw2(9) == 3);
constexpr int sw3(unsigned __int128 v) {
  switch (v) {
  case ~(unsigned __int128)0: return 1;
  case (unsigned __int128)1 << 127: return 2;
  }
  return 3;
}
static_assert(sw3(~(unsigned __int128)0) == 1 && sw3((unsigned __int128)1 << 127) == 2);
static_assert(sw3(1) == 3);
