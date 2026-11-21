// [ACLE] The immediate operands of the SME intrinsics, checked in sema as
// clang's `SemaARM::CheckSMEBuiltinFunctionCall` (`CheckImmediateArg`): the
// ZA tile numbers by element width (`za8` 0, `za16` 0-1, `za32` 0-3,
// `za64` 0-7, `za128` 0-15), a tile slice's tile, the ZA tile mask
// (0-255) and the ZT0 register number (0), at their bounds (accepted) and
// past them, an operand that is no integer constant, and a value-dependent
// tile number checked at instantiation. Under `-fsyntax-only`
// (compile-fail). The aliases are declared as <arm_sme.h> declares them.
// spec/sema.md "SVE and SME builtin calls" (WP-1792).
//@ expect: compile-fail
//@ args: --target=aarch64-unknown-linux-gnu -march=armv9-a+sme2 -ferror-limit=0
//@ strict-diags
//@ diag: 46:61 error "argument value 1 is outside the valid range [0, 0]"
//@ diag: 48:3 error "argument value 1 is outside the valid range [0, 0]"
//@ diag: 49:3 error "argument value 2 is outside the valid range [0, 1]"
//@ diag: 50:3 error "argument value 8 is outside the valid range [0, 7]"
//@ diag: 51:3 error "argument value 16 is outside the valid range [0, 15]"
//@ diag: 52:3 error "argument value 4 is outside the valid range [0, 3]"
//@ diag: 53:3 error "argument value 1 is outside the valid range [0, 0]"
//@ diag: 54:3 error "argument value 256 is outside the valid range [0, 255]"
//@ diag: 55:3 error "argument value 18446744073709551615 is outside the valid range [0, 255]"
//@ diag: 56:3 error "argument to 'svld1_hor_za8' must be a constant integer"
//@ diag: 58:71 error "argument value 2 is outside the valid range [0, 1]"
//@ diag: 60:15 note "in instantiation of function template specialization 'dep<2>' requested here"
typedef unsigned long uint64_t;
typedef unsigned int uint32_t;
typedef __SVBool_t svbool_t;
typedef __SVInt8_t svint8_t;
typedef __SVInt32_t svint32_t;
#define M(B) __attribute__((__clang_arm_builtin_alias(__builtin_sme_##B)))
#define ZA __arm_streaming __arm_inout("za")
M(svld1_hor_za8) void svld1_hor_za8(uint64_t, uint32_t, svbool_t, void const *) ZA;
M(svld1_hor_za16) void svld1_hor_za16(uint64_t, uint32_t, svbool_t, void const *) ZA;
M(svld1_hor_za64) void svld1_hor_za64(uint64_t, uint32_t, svbool_t, void const *) ZA;
M(svld1_hor_za128) void svld1_hor_za128(uint64_t, uint32_t, svbool_t, void const *) ZA;
M(svaddha_za32_s32_m) void svaddha_za32_s32_m(uint64_t, svbool_t, svbool_t, svint32_t) ZA;
M(svread_hor_za8_s8_m) svint8_t svread_hor_za8_s8_m(svint8_t, svbool_t, uint64_t, uint32_t) ZA;
M(svzero_mask_za) void svzero_mask_za(uint64_t) __arm_streaming_compatible __arm_inout("za");
M(svzero_zt) void svzero_zt(uint64_t) __arm_streaming_compatible __arm_out("zt0");
void ok(svbool_t p, svint8_t b, svint32_t w, const void *m, uint32_t s) ZA {
  svld1_hor_za8(0, s, p, m); svld1_hor_za16(1, s, p, m); svld1_hor_za64(7, s, p, m);
  svld1_hor_za128(15, s, p, m); svaddha_za32_s32_m(3, p, p, w);
  svread_hor_za8_s8_m(b, p, 0, s); svzero_mask_za(0); svzero_mask_za(255);
}
void zt() __arm_streaming_compatible __arm_out("zt0") { svzero_zt(0); }
void zt_bad() __arm_streaming_compatible __arm_out("zt0") { svzero_zt(1); }
void bad(svbool_t p, svint8_t b, svint32_t w, const void *m, uint32_t s, int k) ZA {
  svld1_hor_za8(1, s, p, m);
  svld1_hor_za16(2, s, p, m);
  svld1_hor_za64(8, s, p, m);
  svld1_hor_za128(16, s, p, m);
  svaddha_za32_s32_m(4, p, p, w);
  svread_hor_za8_s8_m(b, p, 1, s);
  svzero_mask_za(256);
  svzero_mask_za(-1);
  svld1_hor_za8(k, s, p, m);
}
template <int T> void dep(svbool_t p, const void *m, uint32_t s) ZA { svld1_hor_za16(T, s, p, m); }
template void dep<1>(svbool_t, const void *, uint32_t) ZA;
template void dep<2>(svbool_t, const void *, uint32_t) ZA;
