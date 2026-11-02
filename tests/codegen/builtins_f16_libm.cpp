// WP-403: clang's `__fp16` libm builtins lower as their `f` siblings with
// `half` for `float` (clang EmitBuiltinExpr, -O0, AArch64 and x86_64
// alike): the rounding, sign and min/max ones and fma to the `.f16`
// intrinsics (`llvm.roundeven` and `llvm.canonicalize` too), frexpf16 to
// `llvm.frexp.f16.i32`, and the rest to calls of the `half` library
// function the builtin names (`sinf16`, `fmodf16`, `ldexpf16`, ...). The
// float operands are truncated to half and the result extended back.
// Constant operands of the folded fmaxf16 give a constant; fabsf16 is not
// folded (clang does not constant-evaluate it).
float rnd(float x) {
  return __builtin_ceilf16(x) + __builtin_floorf16(x) + __builtin_truncf16(x) +
         __builtin_roundf16(x) + __builtin_roundevenf16(x) + __builtin_rintf16(x) +
         __builtin_canonicalizef16(x);
}
float sgn(float x, float y, float z) {
  return __builtin_fabsf16(x) + __builtin_fmaxf16(x, y) + __builtin_fminf16(x, y) +
         __builtin_fmaf16(x, y, z);
}
float ex(float x, int *e) { return __builtin_frexpf16(x, e); }
float lib(float x, float y, int n) {
  return __builtin_sinf16(x) + __builtin_cosf16(x) + __builtin_tanf16(x) +
         __builtin_expf16(x) + __builtin_exp2f16(x) + __builtin_exp10f16(x) +
         __builtin_logf16(x) + __builtin_log2f16(x) + __builtin_log10f16(x) +
         __builtin_powf16(x, y) + __builtin_fmodf16(x, y) + __builtin_ldexpf16(x, n) +
         __builtin_acosf16(x) + __builtin_asinf16(x) + __builtin_atanf16(x) +
         __builtin_coshf16(x) + __builtin_sinhf16(x) + __builtin_tanhf16(x) +
         __builtin_sqrtf16(x);
}
float folded() { return __builtin_fmaxf16(1.0, 2.0) + __builtin_fabsf16(-1.0) + __builtin_ceilf16(1.5); }
