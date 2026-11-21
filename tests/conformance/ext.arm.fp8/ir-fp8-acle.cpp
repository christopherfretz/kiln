// [ACLE] The FP8 SVE and SME intrinsics (WP-1791, clang 22's; clang 19
// has none): loads, stores, tuples, reinterprets, the `fpm_t` conversions,
// dot products, multiply-adds and matrix multiplies, and the ZA and ZT0
// ones. Each builtin is declared with __clang_arm_builtin_alias, as clang
// 22's <arm_sve.h> and <arm_sme.h> declare it, and called once; clang's
// -O0 lowering is the expected IR (an `fpm_t` operand is a call of
// `llvm.aarch64.set.fpmr` first). spec/landed/codegen/WP-810.md "Builtin
// aliases".
//@ requires: clang>=22
//@ expect: ir
//@ args: --target=aarch64-unknown-linux-gnu -march=armv9.6-a+sve2p1+sme2p1+fp8+fp8dot2+fp8dot4+fp8fma+f8f32mm+sme-f8f16+sme-f8f32
typedef __UINT32_TYPE__ uint32_t;
typedef __UINT64_TYPE__ uint64_t;
typedef __mfp8 mfloat8_t;
typedef uint64_t fpm_t;
typedef __SVBool_t svbool_t;
typedef __SVInt8_t svint8_t;
typedef __SVUint8_t svuint8_t;
typedef __SVUint64_t svuint64_t;
typedef __SVFloat16_t svfloat16_t;
typedef __SVFloat32_t svfloat32_t;
typedef __SVBfloat16_t svbfloat16_t;
typedef __SVMfloat8_t svmfloat8_t;
typedef __clang_svfloat32x2_t svfloat32x2_t;
typedef __clang_svmfloat8x2_t svmfloat8x2_t;
typedef __clang_svmfloat8x4_t svmfloat8x4_t;
#define ALIAS(b) __attribute__((__clang_arm_builtin_alias(b)))
ALIAS(__builtin_sve_svld1_mf8) svmfloat8_t svld1_mf8(svbool_t, mfloat8_t const *);
ALIAS(__builtin_sve_svst1_mf8) void svst1_mf8(svbool_t, mfloat8_t *, svmfloat8_t);
ALIAS(__builtin_sve_svld1q_gather_u64base_mf8)
svmfloat8_t svld1q_gather_u64base_mf8(svbool_t, svuint64_t);
ALIAS(__builtin_sve_svcreate2_mf8) svmfloat8x2_t svcreate2_mf8(svmfloat8_t, svmfloat8_t);
ALIAS(__builtin_sve_svget2_mf8) svmfloat8_t svget2_mf8(svmfloat8x2_t, uint64_t);
ALIAS(__builtin_sve_svundef_mf8) svmfloat8_t svundef_mf8(void);
ALIAS(__builtin_sve_svzipq_mf8_x4) svmfloat8x4_t svzipq_mf8_x4(svmfloat8x4_t);
ALIAS(__builtin_sve_reinterpret_mf8_s8) svmfloat8_t svreinterpret_mf8_s8(svint8_t op);
ALIAS(__builtin_sve_reinterpret_f32_mf8) svfloat32_t svreinterpret_f32_mf8(svmfloat8_t op);
ALIAS(__builtin_sve_svcvt1_f16_mf8_fpm) svfloat16_t svcvt1_f16_mf8_fpm(svmfloat8_t, fpm_t);
ALIAS(__builtin_sve_svcvtlt2_bf16_mf8_fpm)
svbfloat16_t svcvtlt2_bf16_mf8_fpm(svmfloat8_t, fpm_t);
ALIAS(__builtin_sve_svcvtnb_mf8_f32_x2_fpm)
svmfloat8_t svcvtnb_mf8_f32_x2_fpm(svfloat32x2_t, fpm_t);
ALIAS(__builtin_sve_svdot_f16_mf8_fpm)
svfloat16_t svdot_f16_mf8_fpm(svfloat16_t, svmfloat8_t, svmfloat8_t, fpm_t);
ALIAS(__builtin_sve_svdot_lane_f32_mf8_fpm)
svfloat32_t svdot_lane_f32_mf8_fpm(svfloat32_t, svmfloat8_t, svmfloat8_t, uint64_t, fpm_t);
ALIAS(__builtin_sve_svmlalb_f16_mf8_fpm)
svfloat16_t svmlalb_f16_mf8_fpm(svfloat16_t, svmfloat8_t, svmfloat8_t, fpm_t);
ALIAS(__builtin_sve_svmlallbt_lane_f32_mf8_fpm)
svfloat32_t svmlallbt_lane_f32_mf8_fpm(svfloat32_t, svmfloat8_t, svmfloat8_t, uint64_t, fpm_t);
ALIAS(__builtin_sve_svmmla_f32_mf8_fpm)
svfloat32_t svmmla_f32_mf8_fpm(svfloat32_t, svmfloat8_t, svmfloat8_t, fpm_t);
ALIAS(__builtin_sme_svdot_lane_za16_mf8_vg1x2_fpm)
void svdot_lane_za16_mf8_vg1x2_fpm(uint32_t, svmfloat8x2_t, svmfloat8_t, uint64_t, fpm_t);
ALIAS(__builtin_sme_svmla_za32_mf8_vg4x1_fpm)
void svmla_za32_mf8_vg4x1_fpm(uint32_t, svmfloat8_t, svmfloat8_t, fpm_t);
ALIAS(__builtin_sme_svmopa_za32_mf8_m_fpm)
void svmopa_za32_mf8_m_fpm(uint64_t, svbool_t, svbool_t, svmfloat8_t, svmfloat8_t, fpm_t);
ALIAS(__builtin_sme_svread_ver_za8_mf8_m)
svmfloat8_t svread_ver_za8_mf8_m(svmfloat8_t, svbool_t, uint64_t, uint32_t);
ALIAS(__builtin_sme_svwrite_hor_za8_mf8_m)
void svwrite_hor_za8_mf8_m(uint64_t, uint32_t, svbool_t, svmfloat8_t);
ALIAS(__builtin_sme_svreadz_hor_za8_mf8) svmfloat8_t svreadz_hor_za8_mf8(uint64_t, uint32_t);
ALIAS(__builtin_sme_svluti2_lane_zt_mf8)
svmfloat8_t svluti2_lane_zt_mf8(uint64_t, svuint8_t, uint64_t);

svmfloat8_t ld(svbool_t pg, const mfloat8_t *p) { return svld1_mf8(pg, p); }
void st(svbool_t pg, mfloat8_t *p, svmfloat8_t v) { svst1_mf8(pg, p, v); }
svmfloat8_t gather(svbool_t pg, svuint64_t b) { return svld1q_gather_u64base_mf8(pg, b); }
svmfloat8_t tuple(svmfloat8_t a, svmfloat8_t b) { return svget2_mf8(svcreate2_mf8(a, b), 1); }
svmfloat8_t undef() { return svundef_mf8(); }
svmfloat8x4_t zipq(svmfloat8x4_t t) __arm_streaming { return svzipq_mf8_x4(t); }
svfloat32_t reinterpret(svint8_t a) { return svreinterpret_f32_mf8(svreinterpret_mf8_s8(a)); }
svfloat16_t cvt1(svmfloat8_t a, fpm_t fpm) { return svcvt1_f16_mf8_fpm(a, fpm); }
svbfloat16_t cvtlt2(svmfloat8_t a, fpm_t fpm) { return svcvtlt2_bf16_mf8_fpm(a, fpm); }
svmfloat8_t cvtnb(svfloat32x2_t a, fpm_t fpm) { return svcvtnb_mf8_f32_x2_fpm(a, fpm); }
svfloat16_t dot(svfloat16_t z, svmfloat8_t a, svmfloat8_t b, fpm_t fpm) {
  return svdot_f16_mf8_fpm(z, a, b, fpm);
}
svfloat32_t dot_lane(svfloat32_t z, svmfloat8_t a, svmfloat8_t b, fpm_t fpm) {
  return svdot_lane_f32_mf8_fpm(z, a, b, 3, fpm);
}
svfloat16_t mlalb(svfloat16_t z, svmfloat8_t a, svmfloat8_t b, fpm_t fpm) {
  return svmlalb_f16_mf8_fpm(z, a, b, fpm);
}
svfloat32_t mlallbt(svfloat32_t z, svmfloat8_t a, svmfloat8_t b, fpm_t fpm) {
  return svmlallbt_lane_f32_mf8_fpm(z, a, b, 7, fpm);
}
svfloat32_t mmla(svfloat32_t z, svmfloat8_t a, svmfloat8_t b, fpm_t fpm) {
  return svmmla_f32_mf8_fpm(z, a, b, fpm);
}
void za_dot(uint32_t s, svmfloat8x2_t a, svmfloat8_t b, fpm_t fpm) __arm_streaming __arm_inout("za") {
  svdot_lane_za16_mf8_vg1x2_fpm(s, a, b, 1, fpm);
}
void za_mla(uint32_t s, svmfloat8_t a, svmfloat8_t b, fpm_t fpm) __arm_streaming __arm_inout("za") {
  svmla_za32_mf8_vg4x1_fpm(s, a, b, fpm);
}
void za_mopa(svbool_t p, svbool_t q, svmfloat8_t a, svmfloat8_t b, fpm_t fpm) __arm_streaming __arm_inout("za") {
  svmopa_za32_mf8_m_fpm(1, p, q, a, b, fpm);
}
svmfloat8_t za_read(svmfloat8_t z, svbool_t p, uint32_t s) __arm_streaming __arm_in("za") {
  return svread_ver_za8_mf8_m(z, p, 0, s);
}
void za_write(uint32_t s, svbool_t p, svmfloat8_t v) __arm_streaming __arm_inout("za") {
  svwrite_hor_za8_mf8_m(0, s, p, v);
}
svmfloat8_t za_readz(uint32_t s) __arm_streaming __arm_inout("za") { return svreadz_hor_za8_mf8(0, s); }
svmfloat8_t zt_luti2(svuint8_t i) __arm_streaming __arm_in("zt0") { return svluti2_lane_zt_mf8(0, i, 2); }
