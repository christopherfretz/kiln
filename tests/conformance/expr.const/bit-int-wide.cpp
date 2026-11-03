// [expr.const]/5, [basic.fundamental]/1 (C23 6.2.5): x86-64's
// `__BITINT_MAXWIDTH__` is 8388608, so `_BitInt(200)` is a valid type there
// and clang evaluates it in a constant expression. The Oven has no value model
// above 128 bits yet (spec/oven.md "`_BitInt`"): kiln rejects the initializer with
// "kiln: constant evaluation of a _BitInt wider than 128 bits is not supported
// yet". Runtime use and layout already match clang.
//@ args: --target=x86_64-unknown-linux-gnu
//@ xfail(kiln): constant evaluation of _BitInt wider than 128 bits (spec/oven.md, _BitInt)
constexpr _BitInt(200) w = 5;
static_assert(w + 1 == 6);
static_assert(sizeof(_BitInt(200)) == 32 && alignof(_BitInt(200)) == 8);
_BitInt(200) rt = 7;
unsigned _BitInt(129) add(unsigned _BitInt(129) a, unsigned _BitInt(129) b) { return a + b; }
