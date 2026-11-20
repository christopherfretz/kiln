// [expr.const]: the generic bit builtins and the checked arithmetic
// builtins take a wide `_BitInt(N)` (N > 128) operand in a constant
// expression. x86-64 only (aarch64's `__BITINT_MAXWIDTH__` is 128).
//@ args: --target=x86_64-unknown-linux-gnu
//@ expect: compile-ok
//@ xfail(kiln): WP-1742 remainder, a builtin's wide _BitInt argument is requested as a register value (the rv_inner trap)

using S = _BitInt(200);
using U = unsigned _BitInt(200);
constexpr S big = S(1) << 150;

constexpr bool overflows() {
  S r = 0;
  return __builtin_add_overflow(big, big, &r) || r != big * 2;
}
static_assert(!overflows());
static_assert(__builtin_popcountg((U)big + 1) == 2);
static_assert(__builtin_clzg((U)big) == 49);
static_assert(__builtin_ctzg((U)big) == 150);
