// [expr.const]: the alignment builtins on a _BitInt wider than 128 bits
// are constant expressions, as on any integer (clang ExprConstant
// BI__builtin_align_up / _down / __builtin_is_aligned). x86-64 only
// (aarch64's __BITINT_MAXWIDTH__ is 128).
//@ args: --target=x86_64-unknown-linux-gnu
constexpr unsigned _BitInt(256) x = 37;
static_assert(__builtin_align_up(x, 16) == 48);
static_assert(__builtin_align_down(x, 16) == 32);
static_assert(!__builtin_is_aligned(x, 16));
static_assert(__builtin_is_aligned(x << 200, 1024));
constexpr _BitInt(200) y = -37;
static_assert(__builtin_align_down(y, 8) == -40);
