// C23 6.2.5 / [basic.fundamental], [basic.start.static]: wide `_BitInt`
// (N > 128) globals constant-initialized by the Oven (spec/oven.md
// "`_BitInt`"): the frozen value is emitted as an `iN` constant in its
// ABI-sized slot, as clang emits it. Where BITINT_MAXWIDTH allows (x86_64).
// Widths are chosen so the iN store size equals its alloc size (no byte-array
// slot); the byte-array slots are bit_int_wide_record.cpp.
#if __BITINT_MAXWIDTH__ >= 256
using s200 = _BitInt(200);
using u256 = unsigned _BitInt(256);
constexpr u256 pw(u256 b, int e) {
  u256 r = 1;
  while (e--) r *= b;
  return r;
}
constinit u256 g_pow = pw(3, 160);
constinit s200 g_neg = -(s200(1) << 150) - 7;
struct P {
  s200 a;
  u256 b;
};
constinit P g_p{s200(-5), u256(1) << 255};
s200 sum() { return g_neg + g_p.a; }
#endif
