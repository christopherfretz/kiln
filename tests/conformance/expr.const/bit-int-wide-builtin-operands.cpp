// [expr.const]: the generic bit builtins and the checked arithmetic
// builtins take a wide `_BitInt(N)` (N > 128) operand in a constant
// expression. x86-64 only (aarch64's `__BITINT_MAXWIDTH__` is 128).
//@ args: --target=x86_64-unknown-linux-gnu
//@ expect: compile-ok

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

// __builtin_constant_p evaluates a wide operand as any integer's (kiln
// answered false).
static_assert(__builtin_constant_p(big));
static_assert(__builtin_constant_p(big + 1));
S global;
static_assert(!__builtin_constant_p(global));
constexpr bool constant(S v) { return __builtin_constant_p(v); }
static_assert(constant(3));
