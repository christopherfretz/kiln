// [ACLE] SVE2 and SVE2.1 intrinsics: widening and narrowing arithmetic,
// crypto, bit permutes, histograms, svcount_t predicates, multi-vector
// loads and stores.
//
// Each builtin is declared with __clang_arm_builtin_alias, as <arm_sve.h>
// and <arm_sme.h> declare it, and called once; clang's -O0 lowering
// (EmitAArch64SVEBuiltinExpr / EmitAArch64SMEBuiltinExpr) is the expected
// IR. spec/landed/codegen/WP-810.md "Builtin aliases" (WP-1739).
//@ requires: clang<22
//@ expect: ir
//@ args: --target=aarch64-unknown-linux-gnu -march=armv9.5-a+sve2p1+sme2p1+sve2-aes+sve2-sha3+sve2-sm4+sve2-bitperm+f32mm+f64mm+i8mm+bf16+b16b16+sme-f64f64+sme-i16i64+sme-f16f16+sme-b16b16+lut+faminmax+sme-fa64+sme-lutv2
typedef __INT32_TYPE__ int32_t;
typedef __INT64_TYPE__ int64_t;
typedef __INT8_TYPE__ int8_t;
typedef __SVBool_t svbool_t;
typedef __SVCount_t svcount_t;
typedef __SVInt16_t svint16_t;
typedef __SVInt32_t svint32_t;
typedef __SVInt64_t svint64_t;
typedef __SVInt8_t svint8_t;
typedef __SVUint16_t svuint16_t;
typedef __SVUint32_t svuint32_t;
typedef __SVUint64_t svuint64_t;
typedef __SVUint8_t svuint8_t;
typedef __UINT32_TYPE__ uint32_t;
typedef __UINT64_TYPE__ uint64_t;
typedef __attribute__((neon_vector_type(4))) int int32x4_t;
typedef __clang_svint8x2_t svint8x2_t;
typedef __clang_svuint16x2_t svuint16x2_t;
typedef __clang_svuint32x4_t svuint32x4_t;
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svaba_s8)))
svint8_t svaba_s8(svint8_t, svint8_t, svint8_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svaddlb_s32)))
svint32_t svaddlb_s32(svint16_t, svint16_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svqrshrnb_n_s16)))
svint8_t svqrshrnb_n_s16(svint16_t, uint64_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svhistcnt_s32_z)))
svuint32_t svhistcnt_s32_z(svbool_t, svint32_t, svint32_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svmatch_u8)))
svbool_t svmatch_u8(svbool_t, svuint8_t, svuint8_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svwhilerw_s32)))
svbool_t svwhilerw_s32(int32_t const *, int32_t const *);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svtbl2_u16)))
svuint16_t svtbl2_u16(svuint16x2_t, svuint16_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svxar_n_s64)))
svint64_t svxar_n_s64(svint64_t, svint64_t, uint64_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svaesd_u8)))
svuint8_t svaesd_u8(svuint8_t, svuint8_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svsm4e_u32)))
svuint32_t svsm4e_u32(svuint32_t, svuint32_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svbdep_u8)))
svuint8_t svbdep_u8(svuint8_t, svuint8_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svcdot_s32)))
svint32_t svcdot_s32(svint32_t, svint8_t, svint8_t, uint64_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svldnt1_gather_u64base_s64)))
svint64_t svldnt1_gather_u64base_s64(svbool_t, svuint64_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svstnt1_scatter_u64base_index_s64)))
void svstnt1_scatter_u64base_index_s64(svbool_t, svuint64_t, int64_t, svint64_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svwhilelt_c8_s64)))
svcount_t svwhilelt_c8_s64(int64_t, int64_t, uint64_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svptrue_c8)))
svcount_t svptrue_c8();
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svld1_s8_x2)))
svint8x2_t svld1_s8_x2(svcount_t, int8_t const *);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svst1_u32_x4)))
void svst1_u32_x4(svcount_t, uint32_t *, svuint32x4_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svpext_lane_c8)))
svbool_t svpext_lane_c8(svcount_t, uint64_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svcntp_c16)))
uint64_t svcntp_c16(svcount_t, uint64_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svpsel_lane_b8)))
svbool_t svpsel_lane_b8(svbool_t, svbool_t, uint32_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svpsel_lane_c8)))
svcount_t svpsel_lane_c8(svcount_t, svbool_t, uint32_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svdot_s32_s16)))
svint32_t svdot_s32_s16(svint32_t, svint16_t, svint16_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svclamp_s8)))
svint8_t svclamp_s8(svint8_t, svint8_t, svint8_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svrevd_s8_m)))
svint8_t svrevd_s8_m(svint8_t, svbool_t, svint8_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svld1q_gather_u64base_s32)))
svint32_t svld1q_gather_u64base_s32(svbool_t, svuint64_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svaddqv_s32)))
int32x4_t svaddqv_s32(svbool_t, svint32_t);
svint8_t t0(svint8_t a0, svint8_t a1, svint8_t a2) { return svaba_s8(a0, a1, a2); }
svint32_t t1(svint16_t a0, svint16_t a1) { return svaddlb_s32(a0, a1); }
svint8_t t2(svint16_t a0) { return svqrshrnb_n_s16(a0, 1); }
svuint32_t t3(svbool_t a0, svint32_t a1, svint32_t a2) { return svhistcnt_s32_z(a0, a1, a2); }
svbool_t t4(svbool_t a0, svuint8_t a1, svuint8_t a2) { return svmatch_u8(a0, a1, a2); }
svbool_t t5(int32_t const * a0, int32_t const * a1) { return svwhilerw_s32(a0, a1); }
svuint16_t t6(svuint16x2_t a0, svuint16_t a1) { return svtbl2_u16(a0, a1); }
svint64_t t7(svint64_t a0, svint64_t a1) { return svxar_n_s64(a0, a1, 1); }
svuint8_t t8(svuint8_t a0, svuint8_t a1) { return svaesd_u8(a0, a1); }
svuint32_t t9(svuint32_t a0, svuint32_t a1) { return svsm4e_u32(a0, a1); }
svuint8_t t10(svuint8_t a0, svuint8_t a1) { return svbdep_u8(a0, a1); }
svint32_t t11(svint32_t a0, svint8_t a1, svint8_t a2) { return svcdot_s32(a0, a1, a2, 0); }
svint64_t t12(svbool_t a0, svuint64_t a1) { return svldnt1_gather_u64base_s64(a0, a1); }
void t13(svbool_t a0, svuint64_t a1, int64_t a2, svint64_t a3) { svstnt1_scatter_u64base_index_s64(a0, a1, a2, a3); }
svcount_t t14(int64_t a0, int64_t a1) { return svwhilelt_c8_s64(a0, a1, 2); }
svcount_t t15() { return svptrue_c8(); }
svint8x2_t t16(svcount_t a0, int8_t const * a1) { return svld1_s8_x2(a0, a1); }
void t17(svcount_t a0, uint32_t * a1, svuint32x4_t a2) { svst1_u32_x4(a0, a1, a2); }
svbool_t t18(svcount_t a0) { return svpext_lane_c8(a0, 0); }
uint64_t t19(svcount_t a0) { return svcntp_c16(a0, 2); }
svbool_t t20(svbool_t a0, svbool_t a1, uint32_t a2) { return svpsel_lane_b8(a0, a1, a2); }
svcount_t t21(svcount_t a0, svbool_t a1, uint32_t a2) { return svpsel_lane_c8(a0, a1, a2); }
svint32_t t22(svint32_t a0, svint16_t a1, svint16_t a2) { return svdot_s32_s16(a0, a1, a2); }
svint8_t t23(svint8_t a0, svint8_t a1, svint8_t a2) { return svclamp_s8(a0, a1, a2); }
svint8_t t24(svint8_t a0, svbool_t a1, svint8_t a2) { return svrevd_s8_m(a0, a1, a2); }
svint32_t t25(svbool_t a0, svuint64_t a1) { return svld1q_gather_u64base_s32(a0, a1); }
int32x4_t t27(svbool_t a0, svint32_t a1) { return svaddqv_s32(a0, a1); }
