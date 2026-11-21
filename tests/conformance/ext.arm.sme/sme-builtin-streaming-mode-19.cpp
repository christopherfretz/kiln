// [ACLE] The streaming-mode rules as clang 19: `svpmov_lane` is a
// non-streaming SVE2.1 intrinsic, an error in a streaming or
// streaming-compatible function; and a lambda's call operator stands for
// the function its lambda is in (`getCurFunctionDecl()`), so a
// non-streaming intrinsic called from a lambda in a streaming function is
// an error too. The clang 22 twin is sme-builtin-streaming-mode-22.cpp.
// spec/sema.md "SVE and SME builtin calls" (WP-1793).
//@ requires: clang<22
//@ expect: compile-fail
//@ args: --target=aarch64-unknown-linux-gnu -march=armv9-a+sve2p1+sme2p1
//@ strict-diags
//@ diag: 23:52 error "builtin can only be called from a non-streaming function"
//@ diag: 24:63 error "builtin can only be called from a non-streaming function"
//@ diag: 26:39 error "builtin can only be called from a non-streaming function"
typedef unsigned long uint64_t;
typedef __SVBool_t svbool_t;
typedef __SVInt16_t svint16_t;
typedef __SVFloat16_t svfloat16_t;
#define S(B) __attribute__((__clang_arm_builtin_alias(__builtin_sve_##B)))
S(svpmov_lane_s16_m) svint16_t svpmov_lane_s16_m(svint16_t, svbool_t, uint64_t);
S(svtmad_f16) svfloat16_t svtmad_f16(svfloat16_t, svfloat16_t, uint64_t);
void n1(svint16_t h, svbool_t p) { svpmov_lane_s16_m(h, p, 1); }
void s1(svint16_t h, svbool_t p) __arm_streaming { svpmov_lane_s16_m(h, p, 1); }
void c1(svint16_t h, svbool_t p) __arm_streaming_compatible { svpmov_lane_s16_m(h, p, 1); }
void s2(svfloat16_t f) __arm_streaming {
  auto l = [](svfloat16_t g) { return svtmad_f16(g, g, 1); };
  l(f);
}
