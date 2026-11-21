// [ACLE] clang 22's <arm_sve.h> and <arm_sme.h> (WP-1842) included for
// real at the default clang compatibility level and lowered as clang 22:
// the FP8 intrinsics and types they declare (`svmfloat8_t`, `fpm_t`,
// `mfloat8_t`, WP-1791), the `__arm_fpm_init` / `__arm_set_fpm_*`
// helpers and the `mfloat8x8_t` NEON type of clang 22's
// <arm_vector_types.h>, and `__arm_in_streaming_mode`, a builtin alias in
// clang 22's <arm_sme.h>. Host-restricted (no pinned `--target`): the
// headers reach <stdint.h>, and on an x86_64 host clang has no AArch64
// sysroot.
//@ requires: clang>=22
//@ requires: target-aarch64
//@ expect: ir
//@ args: -march=armv9-a+sve2+sme2+fp8+fp8dot2+sme-f8f16
#include <arm_sve.h>
#include <arm_sme.h>
#include <arm_neon.h>
svfloat16_t dot(svfloat16_t z, svmfloat8_t a, svmfloat8_t b, fpm_t fpm) {
  return svdot_f16_mf8_fpm(z, a, b, fpm);
}
svmfloat8_t tuple(svmfloat8_t a, svmfloat8_t b) { return svget2_mf8(svcreate2_mf8(a, b), 1); }
svfloat16_t cvt(svmfloat8_t a, fpm_t fpm) { return svcvt1_f16_mf8_fpm(a, fpm); }
svmfloat8_t ld(svbool_t pg, const mfloat8_t *p) { return svld1_mf8(pg, p); }
mfloat8x8_t neon(mfloat8x8_t a) { return a; }
fpm_t mode() {
  return __arm_set_fpm_src2_format(__arm_set_fpm_src1_format(__arm_fpm_init(), __ARM_FPM_E4M3),
                                   __ARM_FPM_E5M2);
}
void za(svmfloat8_t a, svmfloat8_t b, fpm_t fpm) __arm_streaming __arm_inout("za") {
  svmla_lane_za16_mf8_vg2x1_fpm(0, a, b, 0, fpm);
}
bool streaming() __arm_streaming_compatible { return __arm_in_streaming_mode(); }
svint32_t add(svint32_t a, svint32_t b) { return svadd_s32_z(svptrue_b32(), a, b); }
