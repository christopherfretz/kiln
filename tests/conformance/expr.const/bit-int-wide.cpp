// [expr.const]/5, [basic.fundamental]/1 (C23 6.2.5): x86-64's
// `__BITINT_MAXWIDTH__` is 8388608, so `_BitInt(200)` is a valid type there
// and clang evaluates it in a constant expression; the Oven holds such a value
// in memory as 64-bit limbs (spec/oven.md "`_BitInt`"; the operations are
// bit-int-wide-ops.cpp). Runtime use and layout match clang.
//@ args: --target=x86_64-unknown-linux-gnu
constexpr _BitInt(200) w = 5;
static_assert(w + 1 == 6);
static_assert(sizeof(_BitInt(200)) == 32 && alignof(_BitInt(200)) == 8);
_BitInt(200) rt = 7;
unsigned _BitInt(129) add(unsigned _BitInt(129) a, unsigned _BitInt(129) b) { return a + b; }
