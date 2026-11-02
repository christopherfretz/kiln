// The GNU *f128 math builtins (clang EmitBuiltinExpr): on aarch64
// __float128 is unavailable and they take and return `long double`
// (fp128). The ones with an LLVM intrinsic (ceil, fabs, copysign, fma,
// sqrt, ...) lower to it; the rest call the libm `l` function, as the
// plain long double builtins do. huge_val/inf/nan fold to constants.
using Q = decltype(__builtin_inff128()); int i; long li; Q q, r;
void f() {
 r = __builtin_acosf128(q);
 r = __builtin_acoshf128(q);
 r = __builtin_asinf128(q);
 r = __builtin_asinhf128(q);
 r = __builtin_atanf128(q);
 r = __builtin_atanhf128(q);
 r = __builtin_cbrtf128(q);
 r = __builtin_ceilf128(q);
 r = __builtin_cosf128(q);
 r = __builtin_coshf128(q);
 r = __builtin_erfcf128(q);
 r = __builtin_erff128(q);
 r = __builtin_exp10f128(q);
 r = __builtin_exp2f128(q);
 r = __builtin_expf128(q);
 r = __builtin_expm1f128(q);
 r = __builtin_fabsf128(q);
 r = __builtin_floorf128(q);
 r = __builtin_lgammaf128(q);
 r = __builtin_log10f128(q);
 r = __builtin_log1pf128(q);
 r = __builtin_log2f128(q);
 r = __builtin_logbf128(q);
 r = __builtin_logf128(q);
 r = __builtin_nearbyintf128(q);
 r = __builtin_rintf128(q);
 r = __builtin_roundevenf128(q);
 r = __builtin_roundf128(q);
 r = __builtin_sinf128(q);
 r = __builtin_sinhf128(q);
 r = __builtin_sqrtf128(q);
 r = __builtin_tanf128(q);
 r = __builtin_tanhf128(q);
 r = __builtin_tgammaf128(q);
 r = __builtin_truncf128(q);
 r = __builtin_atan2f128(q, r);
 r = __builtin_copysignf128(q, r);
 r = __builtin_fdimf128(q, r);
 r = __builtin_fmaxf128(q, r);
 r = __builtin_fminf128(q, r);
 r = __builtin_fmodf128(q, r);
 r = __builtin_hypotf128(q, r);
 r = __builtin_nextafterf128(q, r);
 r = __builtin_nexttowardf128(q, r);
 r = __builtin_powf128(q, r);
 r = __builtin_remainderf128(q, r);
 r = __builtin_fmaf128(q, r, q);
 r = __builtin_frexpf128(q, &i);
 i = __builtin_ilogbf128(q);
 r = __builtin_ldexpf128(q, i);
 li = __builtin_llrintf128(q) + __builtin_llroundf128(q) + __builtin_lrintf128(q) + __builtin_lroundf128(q);
 r = __builtin_modff128(q, &r);
 r = __builtin_remquof128(q, r, &i);
 r = __builtin_scalblnf128(q, li);
 r = __builtin_scalbnf128(q, i);
 r = __builtin_huge_valf128();
 q = __builtin_inff128();
 r = __builtin_nanf128("");
 q = __builtin_nansf128("1");
}
