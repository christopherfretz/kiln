// [ACLE] The streaming mode and the ZA/ZT0 state an SVE or SME intrinsic
// needs, checked at the call in sema as clang's `SemaARM::
// checkArmStreamingBuiltin` and `CheckSMEBuiltinFunctionCall`: a
// non-streaming intrinsic (`svtmad`) from a streaming or
// streaming-compatible function, or a streaming one (`svld1_hor_za8`)
// from a non-streaming or streaming-compatible function, is an error; an
// intrinsic valid in either mode (`svext`, `sve|sme`) is checked against
// the caller's own target features (here each function's `target`
// attribute): without `sve` a streaming-compatible caller needs streaming
// mode. A ZA or ZT0 intrinsic from a function without that state (and no
// `__arm_new`) warns. Each rule's accepted case first. spec/sema.md "SVE
// and SME builtin calls" (WP-1793).
//@ expect: compile-fail
//@ args: --target=aarch64-unknown-linux-gnu -ferror-limit=0
//@ strict-diags
//@ diag: 67:10 error "function using ZA state requires 'sme'"
//@ diag: 71:50 error "builtin can only be called from a non-streaming function"
//@ diag: 72:61 error "builtin can only be called from a non-streaming function"
//@ diag: 73:58 error "builtin can only be called from a non-streaming function"
//@ diag: 74:64 error "builtin can only be called from a streaming function"
//@ diag: 75:91 error "builtin can only be called from a streaming function"
//@ diag: 76:63 error "SVE vector type 'svint8_t' (aka '__SVInt8_t') cannot be used in a non-streaming function"
//@ diag: 76:66 error "SVE vector type 'svint8_t' (aka '__SVInt8_t') cannot be used in a non-streaming function"
//@ diag: 76:54 error "SVE vector type 'svint8_t' (aka '__SVInt8_t') cannot be used in a non-streaming function"
//@ diag: 76:54 error "SVE vector type 'svint8_t' (aka '__SVInt8_t') cannot be used in a non-streaming function"
//@ diag: 76:54 error "SVE vector type 'svint8_t' (aka '__SVInt8_t') cannot be used in a non-streaming function"
//@ diag: 76:54 error "builtin can only be called from a streaming function"
//@ diag: 76:10 error "SVE vector type 'svint8_t' (aka '__SVInt8_t') cannot be used in a non-streaming function"
//@ diag: 77:82 error "call to a shared ZA function requires the caller to have ZA state"
//@ diag: 77:58 warning "builtin call is not valid when calling from a function without active ZA state"
//@ diag: 78:60 error "call to a shared ZA function requires the caller to have ZA state"
//@ diag: 78:44 warning "builtin call is not valid when calling from a function without active ZA state"
//@ diag: 79:56 error "call to a shared ZT0 function requires the caller to have ZT0 state"
//@ diag: 79:45 warning "builtin call is not valid when calling from a function without active ZT0 state"
//@ diag: 80:51 error "builtin can only be called from a non-streaming function"
//@ diag: 81:6 error "function executed in streaming-SVE mode requires 'sme'"
//@ diag: 81:49 error "SVE vector type 'svint8_t' (aka '__SVInt8_t') cannot be used in a target without sve"
//@ diag: 81:52 error "SVE vector type 'svint8_t' (aka '__SVInt8_t') cannot be used in a target without sve"
//@ diag: 81:40 error "SVE vector type 'svint8_t' (aka '__SVInt8_t') cannot be used in a target without sve"
//@ diag: 81:40 error "SVE vector type 'svint8_t' (aka '__SVInt8_t') cannot be used in a target without sve"
//@ diag: 81:40 error "SVE vector type 'svint8_t' (aka '__SVInt8_t') cannot be used in a target without sve"
//@ diag: 81:6 error "SVE vector type 'svint8_t' (aka '__SVInt8_t') cannot be used in a target without sve"
typedef unsigned long uint64_t;
typedef unsigned int uint32_t;
typedef __SVBool_t svbool_t;
typedef __SVInt8_t svint8_t;
typedef __SVFloat16_t svfloat16_t;
#define S(B) __attribute__((__clang_arm_builtin_alias(__builtin_sve_##B)))
#define M(B) __attribute__((__clang_arm_builtin_alias(__builtin_sme_##B)))
S(svext_s8) svint8_t svext_s8(svint8_t, svint8_t, uint64_t);
S(svtmad_f16) svfloat16_t svtmad_f16(svfloat16_t, svfloat16_t, uint64_t);
M(svld1_hor_za8) void svld1_hor_za8(uint64_t, uint32_t, svbool_t, void const *) __arm_streaming __arm_inout("za");
M(svzero_mask_za) void svzero_mask_za(uint64_t) __arm_streaming_compatible __arm_inout("za");
M(svcntsb) uint64_t svcntsb(void) __arm_streaming_compatible;
M(svzero_zt) void svzero_zt(uint64_t) __arm_streaming_compatible __arm_out("zt0");
#define SVE __attribute__((target("sve")))
#define SME __attribute__((target("sme")))
#define SVE_SME __attribute__((target("sve,sme")))
#define SME2 __attribute__((target("sme2")))
SVE void n1(svint8_t b, svfloat16_t f) { svext_s8(b, b, 1); svtmad_f16(f, f, 1); }
SME void s1(svint8_t b) __arm_streaming { svext_s8(b, b, 1); }
SME __arm_locally_streaming void s2(svint8_t b) { svext_s8(b, b, 1); }
SVE_SME void c1(svint8_t b) __arm_streaming_compatible { svext_s8(b, b, 1); }
SME void s3(svbool_t p, const void *m) __arm_streaming __arm_inout("za") { svld1_hor_za8(0, 0, p, m); }
SME __arm_new("za") void s4(svbool_t p, const void *m) __arm_streaming { svld1_hor_za8(0, 0, p, m); }
SME void c2() __arm_streaming_compatible __arm_inout("za") { svzero_mask_za(1); }
SVE void n2() __arm_inout("za") { svzero_mask_za(1); }
uint64_t n3() { return svcntsb(); }
SME2 void c3() __arm_streaming_compatible __arm_out("zt0") { svzero_zt(0); }
SME2 __arm_new("zt0") void n4() { svzero_zt(0); }
SVE_SME void e1(svfloat16_t f) __arm_streaming { svtmad_f16(f, f, 1); }
SVE_SME void e2(svfloat16_t f) __arm_streaming_compatible { svtmad_f16(f, f, 1); }
SVE_SME __arm_locally_streaming void e3(svfloat16_t f) { svtmad_f16(f, f, 1); }
SVE_SME void e4(svbool_t p, const void *m) __arm_inout("za") { svld1_hor_za8(0, 0, p, m); }
SVE_SME void e5(svbool_t p, const void *m) __arm_streaming_compatible __arm_inout("za") { svld1_hor_za8(0, 0, p, m); }
SME void e6(svint8_t b) __arm_streaming_compatible { svext_s8(b, b, 1); }
SME void e7(svbool_t p, const void *m) __arm_streaming { svld1_hor_za8(0, 0, p, m); }
SME void e8() __arm_streaming_compatible { svzero_mask_za(1); }
SME2 void e9() __arm_streaming_compatible { svzero_zt(0); }
SVE_SME void e10(svfloat16_t f) __arm_streaming { svtmad_f16(f, f, 99); }
void e11(svint8_t b) __arm_streaming { svext_s8(b, b, 1); }
