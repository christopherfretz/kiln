// [ACLE] SME and SME2 intrinsics: ZA tiles and slices, ZT0, multi-vector
// ZA arithmetic, streaming-mode queries.
//
// Each builtin is declared with __clang_arm_builtin_alias, as <arm_sve.h>
// and <arm_sme.h> declare it, and called once; clang's -O0 lowering
// (EmitAArch64SVEBuiltinExpr / EmitAArch64SMEBuiltinExpr) is the expected
// IR. spec/landed/codegen/WP-810.md "Builtin aliases" (WP-1739).
//@ requires: clang<22
//@ expect: ir
//@ args: --target=aarch64-unknown-linux-gnu -march=armv9.5-a+sve2p1+sme2p1+sve2-aes+sve2-sha3+sve2-sm4+sve2-bitperm+f32mm+f64mm+i8mm+bf16+b16b16+sme-f64f64+sme-i16i64+sme-f16f16+sme-b16b16+lut+faminmax+sme-fa64+sme-lutv2
typedef __INT64_TYPE__ int64_t;
typedef __SVBool_t svbool_t;
typedef __SVCount_t svcount_t;
typedef __SVFloat32_t svfloat32_t;
typedef __SVInt32_t svint32_t;
typedef __SVInt8_t svint8_t;
typedef __SVUint8_t svuint8_t;
typedef __UINT32_TYPE__ uint32_t;
typedef __UINT64_TYPE__ uint64_t;
typedef __clang_svfloat32x4_t svfloat32x4_t;
typedef __clang_svint32x2_t svint32x2_t;
typedef __clang_svint8x2_t svint8x2_t;
__attribute__((__clang_arm_builtin_alias(__builtin_sme_svzero_za)))
void svzero_za();
__attribute__((__clang_arm_builtin_alias(__builtin_sme_svzero_mask_za)))
void svzero_mask_za(uint64_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sme_svread_hor_za8_s8_m)))
svint8_t svread_hor_za8_s8_m(svint8_t, svbool_t, uint64_t, uint32_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sme_svwrite_ver_za32_f32_m)))
void svwrite_ver_za32_f32_m(uint64_t, uint32_t, svbool_t, svfloat32_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sme_svmopa_za32_f32_m)))
void svmopa_za32_f32_m(uint64_t, svbool_t, svbool_t, svfloat32_t, svfloat32_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sme_svaddha_za32_s32_m)))
void svaddha_za32_s32_m(uint64_t, svbool_t, svbool_t, svint32_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sme_svld1_hor_za8)))
void svld1_hor_za8(uint64_t, uint32_t, svbool_t, void const *);
__attribute__((__clang_arm_builtin_alias(__builtin_sme_svst1_ver_vnum_za32)))
void svst1_ver_vnum_za32(uint64_t, uint32_t, svbool_t, void *, int64_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sme_svldr_za)))
void svldr_za(uint32_t, void const *);
__attribute__((__clang_arm_builtin_alias(__builtin_sme_svstr_vnum_za)))
void svstr_vnum_za(uint32_t, void *, int64_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sme_svcntsb)))
uint64_t svcntsb();
__attribute__((__clang_arm_builtin_alias(__builtin_sme_svmla_za32_s8_vg4x2)))
void svmla_za32_s8_vg4x2(uint32_t, svint8x2_t, svint8x2_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sme_svread_za8_s8_vg1x2)))
svint8x2_t svread_za8_s8_vg1x2(uint32_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sme_svluti2_lane_zt_s8)))
svint8_t svluti2_lane_zt_s8(uint64_t, svuint8_t, uint64_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sme_svzero_zt)))
void svzero_zt(uint64_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sme_svadd_za32_s32_vg1x2)))
void svadd_za32_s32_vg1x2(uint32_t, svint32x2_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sme_svcntsw)))
uint64_t svcntsw();
__attribute__((__clang_arm_builtin_alias(__builtin_sme_svmla_lane_za32_f32_vg1x4)))
void svmla_lane_za32_f32_vg1x4(uint32_t, svfloat32x4_t, svfloat32_t, uint64_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svmax_single_s8_x2)))
svint8x2_t svmax_single_s8_x2(svint8x2_t, svint8_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svsel_s8_x2)))
svint8x2_t svsel_s8_x2(svcount_t, svint8x2_t, svint8x2_t);
void t0() { svzero_za(); }
void t1() { svzero_mask_za(100); }
svint8_t t2(svint8_t a0, svbool_t a1, uint32_t a3) __arm_streaming { return svread_hor_za8_s8_m(a0, a1, 0, a3); }
void t3(uint32_t a1, svbool_t a2, svfloat32_t a3) __arm_streaming { svwrite_ver_za32_f32_m(0, a1, a2, a3); }
void t4(svbool_t a1, svbool_t a2, svfloat32_t a3, svfloat32_t a4) __arm_streaming { svmopa_za32_f32_m(0, a1, a2, a3, a4); }
void t5(svbool_t a1, svbool_t a2, svint32_t a3) __arm_streaming { svaddha_za32_s32_m(0, a1, a2, a3); }
void t6(uint32_t a1, svbool_t a2, void const * a3) __arm_streaming { svld1_hor_za8(0, a1, a2, a3); }
void t7(uint32_t a1, svbool_t a2, void * a3, int64_t a4) __arm_streaming { svst1_ver_vnum_za32(0, a1, a2, a3, a4); }
void t8(uint32_t a0, void const * a1) { svldr_za(a0, a1); }
void t9(uint32_t a0, void * a1, int64_t a2) { svstr_vnum_za(a0, a1, a2); }
uint64_t t10() { return svcntsb(); }
void t11(uint32_t a0, svint8x2_t a1, svint8x2_t a2) __arm_streaming { svmla_za32_s8_vg4x2(a0, a1, a2); }
svint8x2_t t12(uint32_t a0) __arm_streaming { return svread_za8_s8_vg1x2(a0); }
svint8_t t13(svuint8_t a1) __arm_streaming { return svluti2_lane_zt_s8(0, a1, 0); }
void t14() { svzero_zt(0); }
void t15(uint32_t a0, svint32x2_t a1) __arm_streaming { svadd_za32_s32_vg1x2(a0, a1); }
uint64_t t17() { return svcntsw(); }
void t18(uint32_t a0, svfloat32x4_t a1, svfloat32_t a2) __arm_streaming { svmla_lane_za32_f32_vg1x4(a0, a1, a2, 0); }
svint8x2_t t19(svint8x2_t a0, svint8_t a1) __arm_streaming { return svmax_single_s8_x2(a0, a1); }
svint8x2_t t20(svcount_t a0, svint8x2_t a1, svint8x2_t a2) __arm_streaming { return svsel_s8_x2(a0, a1, a2); }
