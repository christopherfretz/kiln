// [ACLE] <arm_sme.h>, <arm_sve.h> and <arm_neon_sve_bridge.h> included for
// real and lowered as clang 22: SVE arithmetic and counts, the NEON-SVE
// bridge, ZA intrinsics (WP-1739), `__arm_has_sme` over
// `__builtin_arm_get_sme_state` (a call of `__arm_sme_state`, WP-1794),
// `svundef_za` and the SME support routines (spec/codegen.md "Target
// builtins"). clang 22's header makes `__arm_in_streaming_mode` a builtin
// alias where kiln's (clang 19's) defines it inline, so only the -19
// sibling calls it. Host-restricted (no pinned `--target`): the headers
// reach <stdint.h>, and on an x86_64 host clang has no AArch64 sysroot.
// `-ftrivial-auto-var-init=uninitialized`: clang has no C++26 erroneous
// values.
//@ requires: clang>=22
//@ requires: target-aarch64
//@ expect: ir
//@ args: -march=armv9-a+sme2 -ftrivial-auto-var-init=uninitialized
#include <arm_sme.h>
#include <arm_neon_sve_bridge.h>
svint32_t add(svint32_t a, svint32_t b) { return svadd_s32_z(svptrue_b32(), a, b); }
svfloat32_t mul(svbool_t pg, svfloat32_t a) { return svmul_x(pg, a, 2.0f); }
uint64_t cnt() { return svcntw(); }
int32x4_t lo(svint32_t a) { return svget_neonq_s32(a); }
svint32_t dup(int32x4_t a) { return svdup_neonq_s32(a); }
svint32_t set(svint32_t a, int32x4_t b) { return svset_neonq(a, b); }
void zero() __arm_streaming __arm_inout("za") { svzero_za(); }
uint64_t cnts() __arm_streaming_compatible { return svcntsw(); }
void wr(svbool_t pg, svint32_t v) __arm_streaming __arm_inout("za") {
  svwrite_hor_za32_s32_m(0, 1, pg, v);
}
bool has() __arm_streaming_compatible { return __arm_has_sme(); }
void undef() __arm_out("za") { svundef_za(); }
void dis() { __arm_za_disable(); }
void *cp(void *d, const void *s, size_t n) __arm_streaming_compatible {
  return __arm_sc_memcpy(d, s, n);
}
