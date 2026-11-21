// [ACLE] SVE data-processing intrinsics: predicated arithmetic (_m, _x,
// _z, _n), reductions, comparisons, conversions, permutes, immediates.
//
// Each builtin is declared with __clang_arm_builtin_alias, as <arm_sve.h>
// and <arm_sme.h> declare it, and called once; clang's -O0 lowering
// (EmitAArch64SVEBuiltinExpr / EmitAArch64SMEBuiltinExpr) is the expected
// IR. spec/landed/codegen/WP-810.md "Builtin aliases" (WP-1739).
//@ requires: clang<22
//@ expect: ir
//@ args: --target=aarch64-unknown-linux-gnu -march=armv9.5-a+sve2p1+sme2p1+sve2-aes+sve2-sha3+sve2-sm4+sve2-bitperm+f32mm+f64mm+i8mm+bf16+b16b16+sme-f64f64+sme-i16i64+sme-f16f16+sme-b16b16+lut+faminmax+sme-fa64+sme-lutv2
typedef __INT16_TYPE__ int16_t;
typedef __INT32_TYPE__ int32_t;
typedef __INT8_TYPE__ int8_t;
typedef __SVBfloat16_t svbfloat16_t;
typedef __SVBool_t svbool_t;
typedef __SVFloat16_t svfloat16_t;
typedef __SVFloat32_t svfloat32_t;
typedef __SVFloat64_t svfloat64_t;
typedef __SVInt16_t svint16_t;
typedef __SVInt32_t svint32_t;
typedef __SVInt64_t svint64_t;
typedef __SVInt8_t svint8_t;
typedef __SVUint16_t svuint16_t;
typedef __SVUint32_t svuint32_t;
typedef __SVUint64_t svuint64_t;
typedef __SVUint8_t svuint8_t;
typedef __UINT64_TYPE__ uint64_t;
typedef __UINT8_TYPE__ uint8_t;
typedef double float64_t;
typedef float float32_t;
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svadd_s32_z)))
svint32_t svadd_s32_z(svbool_t, svint32_t, svint32_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svadd_n_f64_m)))
svfloat64_t svadd_n_f64_m(svbool_t, svfloat64_t, float64_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svmla_f16_x)))
svfloat16_t svmla_f16_x(svbool_t, svfloat16_t, svfloat16_t, svfloat16_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svabd_u8_m)))
svuint8_t svabd_u8_m(svbool_t, svuint8_t, svuint8_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svqadd_n_s16)))
svint16_t svqadd_n_s16(svint16_t, int16_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svdot_s32)))
svint32_t svdot_s32(svint32_t, svint8_t, svint8_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svdot_lane_s32)))
svint32_t svdot_lane_s32(svint32_t, svint8_t, svint8_t, uint64_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svcmpeq_n_s8)))
svbool_t svcmpeq_n_s8(svbool_t, svint8_t, int8_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svcmplt_f32)))
svbool_t svcmplt_f32(svbool_t, svfloat32_t, svfloat32_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svaddv_u16)))
uint64_t svaddv_u16(svbool_t, svuint16_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svmaxnmv_f64)))
float64_t svmaxnmv_f64(svbool_t, svfloat64_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svclz_s32_z)))
svuint32_t svclz_s32_z(svbool_t, svint32_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svcvt_f32_s32_x)))
svfloat32_t svcvt_f32_s32_x(svbool_t, svint32_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svcvt_s64_f16_z)))
svint64_t svcvt_s64_f16_z(svbool_t, svfloat16_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_reinterpret_u8_f32)))
svuint8_t reinterpret_u8_f32(svfloat32_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svdup_n_s8_z)))
svint8_t svdup_n_s8_z(svbool_t, int8_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svdup_lane_u64)))
svuint64_t svdup_lane_u64(svuint64_t, uint64_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svdup_n_b32)))
svbool_t svdup_n_b32(bool);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svext_s8)))
svint8_t svext_s8(svint8_t, svint8_t, uint64_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svinsr_n_f32)))
svfloat32_t svinsr_n_f32(svfloat32_t, float32_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svsel_s16)))
svint16_t svsel_s16(svbool_t, svint16_t, svint16_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svsplice_u32)))
svuint32_t svsplice_u32(svbool_t, svuint32_t, svuint32_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svtbl_s8)))
svint8_t svtbl_s8(svint8_t, svuint8_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svasr_wide_s8_m)))
svint8_t svasr_wide_s8_m(svbool_t, svint8_t, svuint64_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svlsl_n_u8_x)))
svuint8_t svlsl_n_u8_x(svbool_t, svuint8_t, uint8_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svcntp_b16)))
uint64_t svcntp_b16(svbool_t, svbool_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svptest_any)))
bool svptest_any(svbool_t, svbool_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svand_b_z)))
svbool_t svand_b_z(svbool_t, svbool_t, svbool_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svpfalse_b)))
svbool_t svpfalse_b();
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svindex_s32)))
svint32_t svindex_s32(int32_t, int32_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svzip1_s64)))
svint64_t svzip1_s64(svint64_t, svint64_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svmmla_s32)))
svint32_t svmmla_s32(svint32_t, svint8_t, svint8_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svbfdot_f32)))
svfloat32_t svbfdot_f32(svfloat32_t, svbfloat16_t, svbfloat16_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svcadd_f32_x)))
svfloat32_t svcadd_f32_x(svbool_t, svfloat32_t, svfloat32_t, uint64_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svcmla_lane_f16)))
svfloat16_t svcmla_lane_f16(svfloat16_t, svfloat16_t, svfloat16_t, uint64_t, uint64_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svqdech_pat_s16)))
svint16_t svqdech_pat_s16(svint16_t, int, uint64_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svcntb_pat)))
uint64_t svcntb_pat(int);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svlasta_s32)))
int32_t svlasta_s32(svbool_t, svint32_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svclasta_n_f64)))
float64_t svclasta_n_f64(svbool_t, float64_t, svfloat64_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svrev_b8)))
svbool_t svrev_b8(svbool_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svdupq_n_s32)))
svint32_t svdupq_n_s32(int32_t, int32_t, int32_t, int32_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svdupq_n_b8)))
svbool_t svdupq_n_b8(bool, bool, bool, bool, bool, bool, bool, bool, bool, bool, bool, bool, bool, bool, bool, bool);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svdupq_lane_f32)))
svfloat32_t svdupq_lane_f32(svfloat32_t, uint64_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svptrue_pat_b16)))
svbool_t svptrue_pat_b16(int);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svwhilele_b64_u64)))
svbool_t svwhilele_b64_u64(uint64_t, uint64_t);
svint32_t t0(svbool_t a0, svint32_t a1, svint32_t a2) { return svadd_s32_z(a0, a1, a2); }
svfloat64_t t1(svbool_t a0, svfloat64_t a1, float64_t a2) { return svadd_n_f64_m(a0, a1, a2); }
svfloat16_t t2(svbool_t a0, svfloat16_t a1, svfloat16_t a2, svfloat16_t a3) { return svmla_f16_x(a0, a1, a2, a3); }
svuint8_t t3(svbool_t a0, svuint8_t a1, svuint8_t a2) { return svabd_u8_m(a0, a1, a2); }
svint16_t t4(svint16_t a0, int16_t a1) { return svqadd_n_s16(a0, a1); }
svint32_t t5(svint32_t a0, svint8_t a1, svint8_t a2) { return svdot_s32(a0, a1, a2); }
svint32_t t6(svint32_t a0, svint8_t a1, svint8_t a2) { return svdot_lane_s32(a0, a1, a2, 0); }
svbool_t t7(svbool_t a0, svint8_t a1, int8_t a2) { return svcmpeq_n_s8(a0, a1, a2); }
svbool_t t8(svbool_t a0, svfloat32_t a1, svfloat32_t a2) { return svcmplt_f32(a0, a1, a2); }
uint64_t t9(svbool_t a0, svuint16_t a1) { return svaddv_u16(a0, a1); }
float64_t t10(svbool_t a0, svfloat64_t a1) { return svmaxnmv_f64(a0, a1); }
svuint32_t t11(svbool_t a0, svint32_t a1) { return svclz_s32_z(a0, a1); }
svfloat32_t t12(svbool_t a0, svint32_t a1) { return svcvt_f32_s32_x(a0, a1); }
svint64_t t13(svbool_t a0, svfloat16_t a1) { return svcvt_s64_f16_z(a0, a1); }
svuint8_t t14(svfloat32_t a0) { return reinterpret_u8_f32(a0); }
svint8_t t15(svbool_t a0, int8_t a1) { return svdup_n_s8_z(a0, a1); }
svuint64_t t16(svuint64_t a0, uint64_t a1) { return svdup_lane_u64(a0, a1); }
svbool_t t17(bool a0) { return svdup_n_b32(a0); }
svint8_t t18(svint8_t a0, svint8_t a1) { return svext_s8(a0, a1, 102); }
svfloat32_t t19(svfloat32_t a0, float32_t a1) { return svinsr_n_f32(a0, a1); }
svint16_t t20(svbool_t a0, svint16_t a1, svint16_t a2) { return svsel_s16(a0, a1, a2); }
svuint32_t t21(svbool_t a0, svuint32_t a1, svuint32_t a2) { return svsplice_u32(a0, a1, a2); }
svint8_t t22(svint8_t a0, svuint8_t a1) { return svtbl_s8(a0, a1); }
svint8_t t23(svbool_t a0, svint8_t a1, svuint64_t a2) { return svasr_wide_s8_m(a0, a1, a2); }
svuint8_t t24(svbool_t a0, svuint8_t a1, uint8_t a2) { return svlsl_n_u8_x(a0, a1, a2); }
uint64_t t25(svbool_t a0, svbool_t a1) { return svcntp_b16(a0, a1); }
bool t26(svbool_t a0, svbool_t a1) { return svptest_any(a0, a1); }
svbool_t t27(svbool_t a0, svbool_t a1, svbool_t a2) { return svand_b_z(a0, a1, a2); }
svbool_t t28() { return svpfalse_b(); }
svint32_t t29(int32_t a0, int32_t a1) { return svindex_s32(a0, a1); }
svint64_t t30(svint64_t a0, svint64_t a1) { return svzip1_s64(a0, a1); }
svint32_t t31(svint32_t a0, svint8_t a1, svint8_t a2) { return svmmla_s32(a0, a1, a2); }
svfloat32_t t32(svfloat32_t a0, svbfloat16_t a1, svbfloat16_t a2) { return svbfdot_f32(a0, a1, a2); }
svfloat32_t t33(svbool_t a0, svfloat32_t a1, svfloat32_t a2) { return svcadd_f32_x(a0, a1, a2, 90); }
svfloat16_t t34(svfloat16_t a0, svfloat16_t a1, svfloat16_t a2) { return svcmla_lane_f16(a0, a1, a2, 0, 0); }
svint16_t t35(svint16_t a0) { return svqdech_pat_s16(a0, 1, 1); }
uint64_t t36() { return svcntb_pat(1); }
int32_t t37(svbool_t a0, svint32_t a1) { return svlasta_s32(a0, a1); }
float64_t t38(svbool_t a0, float64_t a1, svfloat64_t a2) { return svclasta_n_f64(a0, a1, a2); }
svbool_t t39(svbool_t a0) { return svrev_b8(a0); }
svint32_t t40(int32_t a0, int32_t a1, int32_t a2, int32_t a3) { return svdupq_n_s32(a0, a1, a2, a3); }
svbool_t t41(bool a0, bool a1, bool a2, bool a3, bool a4, bool a5, bool a6, bool a7, bool a8, bool a9, bool a10, bool a11, bool a12, bool a13, bool a14, bool a15) { return svdupq_n_b8(a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14, a15); }
svfloat32_t t42(svfloat32_t a0, uint64_t a1) { return svdupq_lane_f32(a0, a1); }
svbool_t t43() { return svptrue_pat_b16(1); }
svbool_t t44(uint64_t a0, uint64_t a1) { return svwhilele_b64_u64(a0, a1); }
