// GNU `_Complex __bf16` in constant evaluation rounds each part to bf16
// once per operation, the operation computed in `_Complex float` as GCC's
// run-time __mulsc3/__divsc3 and kiln's codegen compute it (WP-380;
// spec/oven.md "Complex values"). GCC 14 is the reference: every
// static_assert below holds under g++-14 (which folds `*`/`/` correctly
// rounded with MPC; with bf16 operands that is the binary32 result here).
// Operations are kept in separate statements: within one expression GCC
// keeps bf16 intermediates in float (excess precision), kiln does not.
//@ expect: compile-ok
//@ strict-diags
//@ skip(clang): clang 19/22 reject `_Complex __bf16`, and fold `__builtin_complex(__bf16, __bf16)` products by rounding every partial product to bf16
typedef decltype(0.0bf16) B;
typedef __complex__ decltype(0.0bf16) CB;
constexpr CB mk(B r, B i) {
  CB z = {r, i};
  return z;
}
// (1.0234375 + i)^2: the real part 1.0234375^2 - 1 = 0.04739379... rounds
// once to 0.04736328125; rounding the partial product 1.0234375^2 to bf16
// first would give 0.046875.
constexpr CB sq = mk(1.0234375bf16, 1.0bf16) * mk(1.0234375bf16, 1.0bf16);
static_assert(__real__ sq == 0.04736328125bf16 && __imag__ sq == 2.046875bf16);
constexpr CB m2 = mk(1.0234375bf16, 1.0bf16) * mk(1.0234375bf16, -1.0bf16);
static_assert(__real__ m2 == 2.046875bf16 && __imag__ m2 == 0.0bf16);
constexpr CB q = mk(1.0bf16, 0.0bf16) / mk(1.0234375bf16, 1.0bf16);
static_assert(__real__ q == 0.5bf16 && __imag__ q == -0.48828125bf16);
// Every operation rounds: 1 + 2^-8 ties to 1 at each step (a single
// addition of 2^-7 would give 1.0078125).
constexpr bool steps() {
  CB a = mk(1.0bf16, 1.0bf16);
  CB b = mk(0.00390625bf16, -0.00390625bf16);
  a = a + b;
  a = a + b;
  if (__real__ a != 1.0bf16 || __imag__ a != 0.9921875bf16)
    return false;
  a = a * mk(1.0234375bf16, 0.0bf16);
  return __real__ a == 1.0234375bf16 && __imag__ a == 1.015625bf16;
}
static_assert(steps());
