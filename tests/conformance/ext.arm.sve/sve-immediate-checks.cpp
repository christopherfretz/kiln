// [ACLE] The immediate operands of the SVE intrinsics, checked in sema as
// clang's `SemaARM::CheckSVEBuiltinFunctionCall` (`CheckImmediateArg`):
// one call per ImmCheck kind at its bounds (accepted) and past them (each
// diagnosed at the call: a value range, a multiple of 2, a rotation of 90
// or 270, or of 0, 90, 180 or 270; a lane index by element width; an
// extract index by element width; a shift amount by element width),
// every failing check of a call reported, an operand that is no integer
// constant, a value-dependent operand checked at instantiation, and a
// constant variable's value. Under `-fsyntax-only` (compile-fail). The
// aliases are declared as <arm_sve.h> declares them. spec/sema.md "SVE and
// SME builtin calls" (WP-1792).
//@ expect: compile-fail
//@ args: --target=aarch64-unknown-linux-gnu -march=armv9-a+sve2p1+sme2 -ferror-limit=0
//@ strict-diags
//@ diag: 88:3 error "argument value 2 is outside the valid range [0, 1]"
//@ diag: 89:3 error "argument value 3 is outside the valid range [0, 2]"
//@ diag: 90:3 error "argument value 18446744073709551615 is outside the valid range [0, 3]"
//@ diag: 91:3 error "argument value 14 is outside the valid range [0, 13]"
//@ diag: 92:3 error "argument value 8 is outside the valid range [0, 7]"
//@ diag: 93:3 error "argument value 0 is outside the valid range [1, 1]"
//@ diag: 94:3 error "argument value 4 is outside the valid range [1, 3]"
//@ diag: 95:3 error "argument value 8 is outside the valid range [1, 7]"
//@ diag: 96:3 error "argument value 0 is outside the valid range [1, 16]"
//@ diag: 97:3 error "argument should be a multiple of 2"
//@ diag: 98:3 error "argument value 6 is outside the valid range [2, 4]"
//@ diag: 99:3 error "argument should be the value 90 or 270"
//@ diag: 100:3 error "argument should be the value 0, 90, 180 or 270"
//@ diag: 101:3 error "argument value 256 is outside the valid range [0, 255]"
//@ diag: 102:3 error "argument value 8 is outside the valid range [0, 7]"
//@ diag: 103:3 error "argument value 4 is outside the valid range [0, 3]"
//@ diag: 103:3 error "argument should be the value 0, 90, 180 or 270"
//@ diag: 104:3 error "argument value 4 is outside the valid range [0, 3]"
//@ diag: 105:3 error "argument value 8 is outside the valid range [0, 7]"
//@ diag: 106:3 error "argument value 0 is outside the valid range [1, 8]"
//@ diag: 107:3 error "argument value 9 is outside the valid range [1, 8]"
//@ diag: 108:3 error "argument to 'svext_s8' must be a constant integer"
//@ diag: 109:3 error "argument to 'svget2_s8' must be a constant integer"
//@ diag: 110:3 error "argument value 9223372036854775808 is outside the valid range [0, 255]"
//@ diag: 112:52 error "argument value 256 is outside the valid range [0, 255]"
//@ diag: 115:19 note "in instantiation of function template specialization 'dep<256>' requested here"
//@ diag: 117:33 error "argument value 300 is outside the valid range [0, 255]"
typedef unsigned long uint64_t;
typedef __SVBool_t svbool_t;
typedef __SVCount_t svcount_t;
typedef __SVInt8_t svint8_t;
typedef __SVInt16_t svint16_t;
typedef __SVInt32_t svint32_t;
typedef __SVInt64_t svint64_t;
typedef __SVFloat16_t svfloat16_t;
typedef __clang_svint8x2_t svint8x2_t;
typedef __clang_svint8x3_t svint8x3_t;
typedef __clang_svint8x4_t svint8x4_t;
enum svpattern { SV_POW2 = 0, SV_ALL = 31 };
enum svprfop { SV_PLDL1KEEP = 0, SV_PSTL3STRM = 13 };
#define A(B) __attribute__((__clang_arm_builtin_alias(__builtin_sve_##B)))
A(svget2_s8) svint8_t svget2_s8(svint8x2_t, uint64_t);
A(svget3_s8) svint8_t svget3_s8(svint8x3_t, uint64_t);
A(svget4_s8) svint8_t svget4_s8(svint8x4_t, uint64_t);
A(svprfb) void svprfb(svbool_t, void const *, enum svprfop);
A(svcntb_pat) uint64_t svcntb_pat(enum svpattern);
A(svtmad_f16) svfloat16_t svtmad_f16(svfloat16_t, svfloat16_t, uint64_t);
A(svpmov_lane_s16_m) svint16_t svpmov_lane_s16_m(svint16_t, svbool_t, uint64_t);
A(svpmov_lane_s32_m) svint32_t svpmov_lane_s32_m(svint32_t, svbool_t, uint64_t);
A(svpmov_lane_s64_m) svint64_t svpmov_lane_s64_m(svint64_t, svbool_t, uint64_t);
A(svqdecd_s64) svint64_t svqdecd_s64(svint64_t, uint64_t);
A(svcntp_c8) uint64_t svcntp_c8(svcount_t, uint64_t);
A(svcadd_f16_m) svfloat16_t svcadd_f16_m(svbool_t, svfloat16_t, svfloat16_t, uint64_t);
A(svcmla_f16_m) svfloat16_t svcmla_f16_m(svbool_t, svfloat16_t, svfloat16_t, svfloat16_t, uint64_t);
A(svext_s8) svint8_t svext_s8(svint8_t, svint8_t, uint64_t);
A(svmla_lane_f16) svfloat16_t svmla_lane_f16(svfloat16_t, svfloat16_t, svfloat16_t, uint64_t);
A(svcmla_lane_f16) svfloat16_t svcmla_lane_f16(svfloat16_t, svfloat16_t, svfloat16_t, uint64_t, uint64_t);
A(svdot_lane_s32) svint32_t svdot_lane_s32(svint32_t, svint8_t, svint8_t, uint64_t);
A(svsli_n_s8) svint8_t svsli_n_s8(svint8_t, svint8_t, uint64_t);
A(svasrd_n_s8_m) svint8_t svasrd_n_s8_m(svbool_t, svint8_t, uint64_t);
A(svshrnb_n_s16) svint8_t svshrnb_n_s16(svint16_t, uint64_t);
void ok(svbool_t p, svcount_t c, svint8_t b, svint16_t h, svint32_t w, svint64_t d,
        svfloat16_t f, svint8x2_t t2, svint8x3_t t3, svint8x4_t t4, const void *m) {
  svget2_s8(t2, 1); svget3_s8(t3, 2); svget4_s8(t4, 3); svprfb(p, m, SV_PSTL3STRM);
  svcntb_pat(SV_ALL); svtmad_f16(f, f, 7); svpmov_lane_s16_m(h, p, 1);
  svpmov_lane_s32_m(w, p, 3); svpmov_lane_s64_m(d, p, 7); svqdecd_s64(d, 16);
  svcntp_c8(c, 2); svcntp_c8(c, 4); svcadd_f16_m(p, f, f, 270); svcmla_f16_m(p, f, f, f, 180);
  svext_s8(b, b, 255); svmla_lane_f16(f, f, f, 7); svcmla_lane_f16(f, f, f, 3, 90);
  svdot_lane_s32(w, b, b, 3); svsli_n_s8(b, b, 0); svsli_n_s8(b, b, 7);
  svasrd_n_s8_m(p, b, 1); svasrd_n_s8_m(p, b, 8); svshrnb_n_s16(h, 8);
}
void bad(svbool_t p, svcount_t c, svint8_t b, svint16_t h, svint32_t w, svint64_t d,
         svfloat16_t f, svint8x2_t t2, svint8x3_t t3, svint8x4_t t4, const void *m, int k) {
  svget2_s8(t2, 2);
  svget3_s8(t3, 3);
  svget4_s8(t4, -1);
  svprfb(p, m, (svprfop)14);
  svtmad_f16(f, f, 8);
  svpmov_lane_s16_m(h, p, 0);
  svpmov_lane_s32_m(w, p, 4);
  svpmov_lane_s64_m(d, p, 8);
  svqdecd_s64(d, 0);
  svcntp_c8(c, 3);
  svcntp_c8(c, 6);
  svcadd_f16_m(p, f, f, 180);
  svcmla_f16_m(p, f, f, f, 45);
  svext_s8(b, b, 256);
  svmla_lane_f16(f, f, f, 8);
  svcmla_lane_f16(f, f, f, 4, 1);
  svdot_lane_s32(w, b, b, 4);
  svsli_n_s8(b, b, 8);
  svasrd_n_s8_m(p, b, 0);
  svshrnb_n_s16(h, 9);
  svext_s8(b, b, k);
  svget2_s8(t2, k);
  svext_s8(b, b, 1ull << 63);
}
template <int N> svint8_t dep(svint8_t b) { return svext_s8(b, b, N); }
template <int N> svint8_t dep2(svint8_t b) { return svext_s8(b, b, N + 1000); }
template svint8_t dep<255>(svint8_t);
template svint8_t dep<256>(svint8_t);
constexpr int K = 300;
svint8_t g(svint8_t b) { return svext_s8(b, b, K); }
