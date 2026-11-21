// [ACLE] <arm_sme.h> (with <arm_sve.h>) compiles and its functions are
// callable: the SME state helpers `__arm_has_sme` and
// `__arm_in_streaming_mode` (inline functions over
// `__builtin_arm_get_sme_state` in clang 19's header, WP-1794), the ZA
// intrinsics (`__clang_arm_builtin_alias` declarations of `__builtin_sme_*`
// builtins, WP-1739), `svundef_za`, and the support routines
// `__arm_za_disable` and `__arm_sc_memcpy`. Host-restricted (no pinned
// `--target`): the header reaches <stdint.h>.
//@ requires: target-aarch64
//@ expect: compile-ok
//@ args: -march=armv9-a+sme2
//@ strict-diags
#include <arm_sme.h>
void f() __arm_streaming __arm_inout("za") { svzero_za(); }
bool g() __arm_streaming_compatible { return __arm_in_streaming_mode(); }
bool h() { return __arm_has_sme(); }
uint64_t n() __arm_streaming_compatible { return svcntsb() + svcntsw(); }
void u() __arm_out("za") { svundef_za(); }
void d() { __arm_za_disable(); }
void *c(void *dst, const void *src, size_t len) __arm_streaming_compatible {
  return __arm_sc_memcpy(dst, src, len);
}
void w(svbool_t pg, svint32_t v) __arm_streaming __arm_inout("za") {
  svwrite_hor_za32_s32_m(0, 1, pg, v);
}
