// [expr.spaceship]/4 (C23 6.2.5): the three-way comparison of two
// `_BitInt(N)` operands with N > 128 is evaluated in a constant expression
// by their exact values (signed or unsigned), giving a
// `std::strong_ordering`. clang is the oracle; x86-64 only (aarch64's
// `__BITINT_MAXWIDTH__` is 128).
//@ args: --target=x86_64-unknown-linux-gnu
#include "../cmp.categories/Inputs/compare.h"
using W = _BitInt(200);
using U = unsigned _BitInt(256);
static_assert((W(1) <=> W(2)) < 0);
static_assert((W(-1) <=> W(-1)) == 0);
static_assert((W(-1) <=> W(1) << 150) < 0);
static_assert(((W(1) << 198) - 1 + (W(1) << 198) <=> -(W(1) << 198)) > 0); // the maximum
static_assert((U(~U(0)) <=> U(1)) > 0);
static_assert((U(1) << 255 <=> U(1) << 255) == 0);
constexpr auto cmp(W a, W b) { return a <=> b; }
static_assert(cmp(W(3) << 170, W(2) << 170) == std::strong_ordering::greater);
static_assert(cmp(-(W(3) << 170), W(2) << 170) == std::strong_ordering::less);
