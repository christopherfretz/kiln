// [class.bit]/1, [expr.ass], [expr.const] (C23 6.7.2.1): a bit-field whose
// declared type is a `_BitInt(N)` with N > 128 is evaluated in a constant
// expression: initialized, assigned, compound-assigned (also with a floating
// computation type), incremented, read back sign- or zero-extended from its
// width and frozen into a constant; a store truncates to the width. A
// constant that does not fit is diagnosed by -Wbitfield-constant-conversion
// with its exact decimal values, also for `__int128`. clang is the oracle;
// x86-64 only (aarch64's `__BITINT_MAXWIDTH__` is 128).
//@ args: --target=x86_64-unknown-linux-gnu
//@ strict-diags
//@ diag: 42:40 warning "implicit truncation from 'W' (aka '_BitInt(200)') to bit-field changes value from 713623846352979940529142984724747568191373312 to -713623846352979940529142984724747568191373312"
//@ diag: 44:39 warning "implicit truncation from 'U' (aka 'unsigned _BitInt(256)') to bit-field changes value from 1393796574908163946345982392040522594123776 to 0"
//@ diag: 50:7 warning "implicit truncation from '__int128' to bit-field changes value from 1298074214633706907132624082305024 to 0"
//@ diag: 52:7 warning "implicit truncation from '__int128' to bit-field changes value from -1267650600228229401496703205376 to 0"
using W = _BitInt(200);
using U = unsigned _BitInt(256);
struct B {
  W x : 150;
  U y : 140;
  int z : 3;
};
constexpr bool bf() {
  B b{};
  b.x = -(W(1) << 140);
  b.y = U(1) << 139;
  b.y++;
  b.z = 3;
  b.y += 2;
  ++b.x;
  return b.x == -(W(1) << 140) + 1 && b.y == (U(1) << 139) + 3 && b.z == 3;
}
static_assert(bf());
constexpr B gb{-(W(1) << 140), 7, 1};
static_assert(gb.x < 0 && gb.y == 7 && gb.z == 1);
constexpr bool fbf() {
  B b{};
  b.x = 2;
  b.x *= 1.5;
  return b.x == 3;
}
static_assert(fbf());
constexpr bool trunc_bf() { B b{}; b.x = W(1) << 149; return b.x == -(W(1) << 149); }
static_assert(trunc_bf());
constexpr bool wrap_bf() { B b{}; b.y = U(1) << 140; b.y = b.y + 5; return b.y == 5; }
static_assert(wrap_bf());
struct S {
  __int128 x : 100;
};
void t(S s) {
  s.x = (__int128)1 << 110;
  s.x = -((__int128)1 << 99); // fits
  s.x = -((__int128)1 << 100);
}
