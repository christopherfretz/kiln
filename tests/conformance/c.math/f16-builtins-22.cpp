// [c.math.syn], [cpp.cond], [expr.const]: the `__fp16` builtins clang 22
// added (atan2f16, sincosf16, fmaximum_numf16, fminimum_numf16; C23
// 7.12.12.5-6 fmaximum_num / fminimum_num) are declared only at
// -fclang-compat=22, with clang 22's types, and fold as clang 22 folds
// them (LLVM `maximumnum` / `minimumnum`: a NaN operand yields the other,
// two NaNs the second one quieted, `-0 < +0`), bit-exactly. So do fmaxf16
// / fminf16 as clang 22 folds them (LLVM 22 `maxnum` / `minnum`: a
// signaling NaN operand, the first one first, yields itself quieted).
// At 19: f16-builtins-19.cpp (WP-403).
//@ expect: compile-ok
//@ strict-diags
//@ args: --target=aarch64-linux-gnu
//@ requires: clang>=22
static_assert(__has_builtin(__builtin_atan2f16) && __has_builtin(__builtin_sincosf16));
static_assert(__has_builtin(__builtin_fmaximum_numf16) && __has_builtin(__builtin_fminimum_numf16));
static_assert(__has_constexpr_builtin(__builtin_fmaximum_numf16));
static_assert(__has_constexpr_builtin(__builtin_fmaxf16));

extern "C" {
__fp16 __builtin_atan2f16(__fp16, __fp16) noexcept;
void __builtin_sincosf16(__fp16, __fp16 *, __fp16 *) noexcept;
__fp16 __builtin_fmaximum_numf16(__fp16, __fp16) noexcept;
__fp16 __builtin_fminimum_numf16(__fp16, __fp16) noexcept;
}

template <class T, class U> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;

void f(__fp16 h, __fp16 *s, __fp16 *c) {
  static_assert(same<decltype(__builtin_atan2f16(h, h)), __fp16>);
  static_assert(same<decltype(__builtin_sincosf16(h, s, c)), void>);
  static_assert(same<decltype(__builtin_fmaximum_numf16(h, h)), __fp16>);
  static_assert(noexcept(__builtin_fminimum_numf16(h, h)));
}

using u16 = unsigned short;
// A macro: x86_64 has no `__fp16` parameters.
#define B(h) __builtin_bit_cast(u16, (__fp16)(h))
constexpr __fp16 snan = __builtin_nansf16("5"); // 0x7c05
constexpr __fp16 qnan = __builtin_nanf16("3");  // 0x7e03
static_assert(B(snan) == 0x7c05 && B(qnan) == 0x7e03);

// fmaximum_num / fminimum_num.
static_assert(B(__builtin_fmaximum_numf16(-0.0, 0.0)) == 0x0000);
static_assert(B(__builtin_fmaximum_numf16(0.0, -0.0)) == 0x0000);
static_assert(B(__builtin_fminimum_numf16(-0.0, 0.0)) == 0x8000);
static_assert(B(__builtin_fminimum_numf16(0.0, -0.0)) == 0x8000);
static_assert(B(__builtin_fmaximum_numf16(qnan, 1.0)) == 0x3c00);
static_assert(B(__builtin_fmaximum_numf16(1.0, qnan)) == 0x3c00);
static_assert(B(__builtin_fmaximum_numf16(snan, 1.0)) == 0x3c00);
static_assert(B(__builtin_fmaximum_numf16(1.0, snan)) == 0x3c00);
static_assert(B(__builtin_fmaximum_numf16(snan, qnan)) == 0x7e03);
static_assert(B(__builtin_fmaximum_numf16(qnan, snan)) == 0x7e05);
static_assert(B(__builtin_fminimum_numf16(snan, snan)) == 0x7e05);
static_assert(B(__builtin_fminimum_numf16(1.5, -2.5)) == 0xc100);
static_assert(B(__builtin_fmaximum_numf16(1.5, -2.5)) == 0x3e00);

// fmax / fmin: a signaling NaN is quieted.
static_assert(B(__builtin_fmaxf16(snan, 1.0)) == 0x7e05);
static_assert(B(__builtin_fmaxf16(1.0, snan)) == 0x7e05);
static_assert(B(__builtin_fmaxf16(qnan, snan)) == 0x7e05);
static_assert(B(__builtin_fminf16(snan, qnan)) == 0x7e05);
static_assert(B(__builtin_fmaxf16(qnan, 1.0)) == 0x3c00);
static_assert(B(__builtin_fminf16(1.0, qnan)) == 0x3c00);
// The float form folds the same way.
static_assert(__builtin_bit_cast(unsigned, __builtin_fmaxf(__builtin_nansf("5"), 1.0f)) == 0x7fc00005);
