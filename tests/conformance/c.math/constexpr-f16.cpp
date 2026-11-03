// [c.math.syn] (P0533 for every floating-point type), [c.math.abs],
// [basic.extended.fp]: clang's `__fp16` builtins of the P0533 functions
// (ceil, floor, trunc, round, roundeven, rint, fmod, fma, frexp, ldexp,
// sqrt) and fabsf16 are constant expressions like their `f` siblings,
// exact in binary16 (a single rounding to half; signed zero and quiet
// NaN payloads kept), as the Oven folds `_Float16` (WP-365/WP-403).
// fmaxf16 / fminf16 also fold in clang (f16-builtins-19.cpp / -22.cpp);
// sin, exp, pow, canonicalize & co. fold nowhere (constexpr-f16-errors.cpp).
// Expectations from the binary16 format's definition.
//@ expect: compile-ok
//@ strict-diags
//@ skip(clang): clang 19/22 constant-evaluate only fmaxf16/fminf16 of these
using u16 = unsigned short;
// A macro: x86_64 has no `__fp16` parameters.
#define B(h) __builtin_bit_cast(u16, (__fp16)(h))
constexpr __fp16 qnan = __builtin_nanf16("3"); // 0x7e03

static_assert(B(__builtin_ceilf16(1.25)) == 0x4000);
static_assert(B(__builtin_ceilf16(-0.5)) == 0x8000);
static_assert(B(__builtin_ceilf16(qnan)) == 0x7e03);
static_assert(B(__builtin_floorf16(-1.5)) == 0xc000);
static_assert(B(__builtin_truncf16(-1.5)) == 0xbc00);
static_assert(B(__builtin_roundf16(2.5)) == 0x4200);
static_assert(B(__builtin_roundevenf16(2.5)) == 0x4000);
static_assert(B(__builtin_rintf16(3.5)) == 0x4400);
static_assert(B(__builtin_fmodf16(5.5, 2)) == 0x3e00);
static_assert(B(__builtin_fmodf16(-5.5, 2)) == 0xbe00);
static_assert(B(__builtin_fmaf16(2, 3, 1)) == 0x4700);
static_assert(B(__builtin_ldexpf16(1, 3)) == 0x4800);
static_assert(B(__builtin_ldexpf16(1, -24)) == 0x0001);
static_assert(B(__builtin_sqrtf16(2)) == 0x3da8);
static_assert(B(__builtin_sqrtf16(-0.0)) == 0x8000);
static_assert(B(__builtin_fabsf16(-0.0)) == 0x0000);
static_assert(B(__builtin_fabsf16(-2.5)) == 0x4100);
constexpr __fp16 nqnan = __builtin_copysignf16(qnan, -1);
static_assert(B(nqnan) == 0xfe03 && B(__builtin_fabsf16(nqnan)) == 0x7e03);

constexpr int frexp_exp(float x) {
  int e = 0;
  (void)__builtin_frexpf16(x, &e);
  return e;
}
static_assert(frexp_exp(8) == 4 && frexp_exp(0x1p-24) == -23);
constexpr float frac(float x) {
  int e = 0;
  return __builtin_frexpf16(x, &e);
}
static_assert(B(frac(8)) == 0x3800 && B(frac(-3)) == 0xba00);
