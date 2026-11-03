//@ min-oracle: 22
// WP-410: clang 22's `__fp16` builtins that clang 19 does not declare
// (-O0, AArch64 and x86_64 alike), lowered as their `f` siblings with
// `half` for `float`: fmaximum_numf16 / fminimum_numf16 to
// `llvm.maximumnum.f16` / `llvm.minimumnum.f16` (folded when constant),
// atan2f16 to a call of `atan2f16`, sincosf16 to a call of `sincosf16`
// with `half` pointers (an `__fp16` parameter is rejected on x86_64).
// The guard keeps the case valid under clang 19.
#if __has_builtin(__builtin_fmaximum_numf16) && __has_builtin(__builtin_sincosf16)
float mm(float x, float y) {
  return __builtin_fmaximum_numf16(x, y) + __builtin_fminimum_numf16(x, y);
}
float at(float x, float y) { return __builtin_atan2f16(x, y); }
void sc(float x, __fp16 *sn, __fp16 *cs) { __builtin_sincosf16(x, sn, cs); }
float folded() {
  return __builtin_fmaximum_numf16(1.0, 2.0) + __builtin_fminimum_numf16(__builtin_nanf(""), 3.0f);
}
#endif
