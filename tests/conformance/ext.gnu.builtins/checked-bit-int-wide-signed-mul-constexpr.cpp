// [GNU] __builtin_mul_overflow takes operands and a result of any
// integer type, also a signed _BitInt wider than 128 bits, in a constant
// expression as at run time (the result wrapped, the flag whether the
// exact product fits). x86-64 only (aarch64's __BITINT_MAXWIDTH__ is
// 128). Deviation (docs/status/deviations.md): clang 19 and 22 reject a
// signed _BitInt operand or result of more than 128 bits ("does not
// support 'signed _BitInt' operands of more than 128 bits"), a limit of
// their run-time library; GCC documents the builtins for every integer
// type and has no _BitInt in C++.
//@ args: --target=x86_64-unknown-linux-gnu
//@ expect: compile-ok
//@ skip(clang): clang rejects a signed _BitInt wider than 128 bits here
using S = _BitInt(200);
constexpr bool mul(S a, S b, S want, bool ovf) {
  S r = 0;
  return __builtin_mul_overflow(a, b, &r) == ovf && r == want;
}
static_assert(mul(S(1) << 150, S(1) << 48, S(1) << 198, false));
static_assert(mul(S(1) << 150, S(1) << 49, S(-1) << 199, true));
static_assert(mul(S(-1) << 150, S(1) << 49, S(-1) << 199, false));
static_assert(mul(S(1) << 150, S(1) << 50, 0, true));
static_assert(mul(-S(3), S(1) << 197, S(-3) << 197, false));
static_assert(mul(-S(5), S(1) << 197, S(3) << 197, true));
constexpr bool narrow() {
  long long r = 0, q = 0;
  return __builtin_mul_overflow(S(-1) << 100, S(1), &r) && r == 0 &&
         !__builtin_mul_overflow(S(-7), S(9), &q) && q == -63;
}
static_assert(narrow());
