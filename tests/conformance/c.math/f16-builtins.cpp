// [c.math.syn], [cpp.cond] (`__has_builtin`): clang's `__fp16` (`h`)
// libm builtins (`Builtins.td` `F16F128` variants) are declared with
// exactly clang's types: `__fp16` parameters and result, `int *` for
// frexpf16's exponent, `int` for ldexpf16's, all `noexcept`. A redeclaration
// with the builtin's type is accepted (clang diagnoses any other type, so
// the clang self-check pins the types); the calls pin kiln's result types.
// clang 19 and 22 agree on all of these; the ones only clang 22 declares
// are f16-builtins-22.cpp / f16-builtins-19.cpp (WP-403). AArch64: x86_64
// has no `__fp16` parameters or results (clang: "parameters cannot have
// __fp16 type"), only the builtins' own.
//@ expect: compile-ok
//@ strict-diags
//@ args: --target=aarch64-linux-gnu
#define HAS(b) static_assert(__has_builtin(b), #b)
HAS(__builtin_acosf16); HAS(__builtin_asinf16); HAS(__builtin_atanf16);
HAS(__builtin_canonicalizef16); HAS(__builtin_ceilf16); HAS(__builtin_copysignf16);
HAS(__builtin_cosf16); HAS(__builtin_coshf16); HAS(__builtin_expf16);
HAS(__builtin_exp10f16); HAS(__builtin_exp2f16); HAS(__builtin_fabsf16);
HAS(__builtin_floorf16); HAS(__builtin_fmaf16); HAS(__builtin_fmaxf16);
HAS(__builtin_fminf16); HAS(__builtin_fmodf16); HAS(__builtin_frexpf16);
HAS(__builtin_ldexpf16); HAS(__builtin_logf16); HAS(__builtin_log10f16);
HAS(__builtin_log2f16); HAS(__builtin_powf16); HAS(__builtin_rintf16);
HAS(__builtin_roundevenf16); HAS(__builtin_roundf16); HAS(__builtin_sinf16);
HAS(__builtin_sinhf16); HAS(__builtin_sqrtf16); HAS(__builtin_tanf16);
HAS(__builtin_tanhf16); HAS(__builtin_truncf16);
// clang has no `f16` nearbyint (nor GCC's other `f16` forms).
static_assert(!__has_builtin(__builtin_nearbyintf16));
static_assert(!__has_builtin(__builtin_hypotf16));

extern "C" {
__fp16 __builtin_acosf16(__fp16) noexcept;
__fp16 __builtin_canonicalizef16(__fp16) noexcept;
__fp16 __builtin_ceilf16(__fp16) noexcept;
__fp16 __builtin_fabsf16(__fp16) noexcept;
__fp16 __builtin_fmaf16(__fp16, __fp16, __fp16) noexcept;
__fp16 __builtin_fmaxf16(__fp16, __fp16) noexcept;
__fp16 __builtin_fmodf16(__fp16, __fp16) noexcept;
__fp16 __builtin_frexpf16(__fp16, int *) noexcept;
__fp16 __builtin_ldexpf16(__fp16, int) noexcept;
__fp16 __builtin_powf16(__fp16, __fp16) noexcept;
__fp16 __builtin_roundevenf16(__fp16) noexcept;
__fp16 __builtin_sinf16(__fp16) noexcept;
}

template <class T, class U> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;

void f(__fp16 h, int n, int *e) {
#define UN(b) static_assert(same<decltype(b(h)), __fp16> && noexcept(b(h)), #b)
  UN(__builtin_acosf16); UN(__builtin_asinf16); UN(__builtin_atanf16);
  UN(__builtin_canonicalizef16); UN(__builtin_ceilf16); UN(__builtin_cosf16);
  UN(__builtin_coshf16); UN(__builtin_expf16); UN(__builtin_exp10f16);
  UN(__builtin_exp2f16); UN(__builtin_fabsf16); UN(__builtin_floorf16);
  UN(__builtin_logf16); UN(__builtin_log10f16); UN(__builtin_log2f16);
  UN(__builtin_rintf16); UN(__builtin_roundevenf16); UN(__builtin_roundf16);
  UN(__builtin_sinf16); UN(__builtin_sinhf16); UN(__builtin_sqrtf16);
  UN(__builtin_tanf16); UN(__builtin_tanhf16); UN(__builtin_truncf16);
#define BIN(b) static_assert(same<decltype(b(h, h)), __fp16> && noexcept(b(h, h)), #b)
  BIN(__builtin_copysignf16); BIN(__builtin_fmaxf16); BIN(__builtin_fminf16);
  BIN(__builtin_fmodf16); BIN(__builtin_powf16);
  static_assert(same<decltype(__builtin_fmaf16(h, h, h)), __fp16>);
  static_assert(same<decltype(__builtin_frexpf16(h, e)), __fp16>);
  static_assert(same<decltype(__builtin_ldexpf16(h, n)), __fp16>);
  // The arguments convert to `__fp16`.
  (void)__builtin_ceilf16(1.5);
  (void)__builtin_fmaxf16(1, 2.0f);
  (void)__builtin_frexpf16(1.5f16, e);
}
