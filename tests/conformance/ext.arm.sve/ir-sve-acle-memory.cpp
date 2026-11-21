// [ACLE] SVE load, store and prefetch intrinsics: contiguous, _vnum,
// extending, gathers and scatters, first-faulting, non-temporal,
// structure loads and stores, replicating loads, the FFR.
//
// Each builtin is declared with __clang_arm_builtin_alias, as <arm_sve.h>
// and <arm_sme.h> declare it, and called once; clang's -O0 lowering
// (EmitAArch64SVEBuiltinExpr / EmitAArch64SMEBuiltinExpr) is the expected
// IR. spec/landed/codegen/WP-810.md "Builtin aliases" (WP-1739).
//@ requires: clang>=22
//@ expect: ir
//@ args: --target=aarch64-unknown-linux-gnu -march=armv9.6-a+sve2p2+sme2p2+sve-aes+sve-sha3+sve-sm4+sve-bitperm+f32mm+f64mm+i8mm+bf16+sve-b16b16+sme-f64f64+sme-i16i64+sme-f16f16+sme-b16b16+sme-lutv2+lut+faminmax+sve-bfscale+sme-mop4+sme-tmop+sve-aes2+ssve-aes+sve-f16f32mm+ssve-bitperm+sme2p1+sve2p1+sme-fa64
typedef __INT16_TYPE__ int16_t;
typedef __INT32_TYPE__ int32_t;
typedef __INT64_TYPE__ int64_t;
typedef __INT8_TYPE__ int8_t;
typedef __SVBool_t svbool_t;
typedef __SVFloat16_t svfloat16_t;
typedef __SVFloat32_t svfloat32_t;
typedef __SVFloat64_t svfloat64_t;
typedef __SVInt32_t svint32_t;
typedef __SVInt64_t svint64_t;
typedef __SVInt8_t svint8_t;
typedef __SVUint16_t svuint16_t;
typedef __SVUint32_t svuint32_t;
typedef __SVUint64_t svuint64_t;
typedef __UINT16_TYPE__ uint16_t;
typedef __UINT8_TYPE__ uint8_t;
typedef __fp16 float16_t;
typedef double float64_t;
typedef float float32_t;
typedef __clang_svfloat64x4_t svfloat64x4_t;
typedef __clang_svint16x2_t svint16x2_t;
typedef __clang_svint32x2_t svint32x2_t;
typedef __clang_svuint8x3_t svuint8x3_t;
typedef __clang_svuint8x4_t svuint8x4_t;
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svld1_vnum_s32)))
svint32_t svld1_vnum_s32(svbool_t, int32_t const *, int64_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svld1sb_s32)))
svint32_t svld1sb_s32(svbool_t, int8_t const *);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svld1_gather_u32base_offset_s32)))
svint32_t svld1_gather_u32base_offset_s32(svbool_t, svuint32_t, int64_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svld1_gather_s64index_f64)))
svfloat64_t svld1_gather_s64index_f64(svbool_t, float64_t const *, svint64_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svldff1_u16)))
svuint16_t svldff1_u16(svbool_t, uint16_t const *);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svldnf1_s8)))
svint8_t svldnf1_s8(svbool_t, int8_t const *);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svldnt1_f32)))
svfloat32_t svldnt1_f32(svbool_t, float32_t const *);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svld2_s16)))
svint16x2_t svld2_s16(svbool_t, int16_t const *);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svld3_u8)))
svuint8x3_t svld3_u8(svbool_t, uint8_t const *);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svld4_f64)))
svfloat64x4_t svld4_f64(svbool_t, float64_t const *);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svst1_vnum_f16)))
void svst1_vnum_f16(svbool_t, float16_t *, int64_t, svfloat16_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svst1h_scatter_u32offset_s32)))
void svst1h_scatter_u32offset_s32(svbool_t, int16_t *, svuint32_t, svint32_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svst2_s32)))
void svst2_s32(svbool_t, int32_t *, svint32x2_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svst4_u8)))
void svst4_u8(svbool_t, uint8_t *, svuint8x4_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svstnt1_s64)))
void svstnt1_s64(svbool_t, int64_t *, svint64_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svprfb)))
void svprfb(svbool_t, void const *, int);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svprfh_vnum)))
void svprfh_vnum(svbool_t, void const *, int64_t, int);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svprfd_gather_u64base)))
void svprfd_gather_u64base(svbool_t, svuint64_t, int);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svld1rq_s32)))
svint32_t svld1rq_s32(svbool_t, int32_t const *);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svld1ro_f32)))
svfloat32_t svld1ro_f32(svbool_t, float32_t const *);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svwrffr)))
void svwrffr(svbool_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svrdffr_z)))
svbool_t svrdffr_z(svbool_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svsetffr)))
void svsetffr();
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svadrb_u32base_s32offset)))
svuint32_t svadrb_u32base_s32offset(svuint32_t, svint32_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svld1ub_gather_u64offset_s64)))
svint64_t svld1ub_gather_u64offset_s64(svbool_t, uint8_t const *, svuint64_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svldff1_gather_u32base_index_f32)))
svfloat32_t svldff1_gather_u32base_index_f32(svbool_t, svuint32_t, int64_t);
svint32_t t0(svbool_t a0, int32_t const * a1, int64_t a2) { return svld1_vnum_s32(a0, a1, a2); }
svint32_t t1(svbool_t a0, int8_t const * a1) { return svld1sb_s32(a0, a1); }
svint32_t t2(svbool_t a0, svuint32_t a1, int64_t a2) { return svld1_gather_u32base_offset_s32(a0, a1, a2); }
svfloat64_t t3(svbool_t a0, float64_t const * a1, svint64_t a2) { return svld1_gather_s64index_f64(a0, a1, a2); }
svuint16_t t4(svbool_t a0, uint16_t const * a1) { return svldff1_u16(a0, a1); }
svint8_t t5(svbool_t a0, int8_t const * a1) { return svldnf1_s8(a0, a1); }
svfloat32_t t6(svbool_t a0, float32_t const * a1) { return svldnt1_f32(a0, a1); }
svint16x2_t t7(svbool_t a0, int16_t const * a1) { return svld2_s16(a0, a1); }
svuint8x3_t t8(svbool_t a0, uint8_t const * a1) { return svld3_u8(a0, a1); }
svfloat64x4_t t9(svbool_t a0, float64_t const * a1) { return svld4_f64(a0, a1); }
void t10(svbool_t a0, float16_t * a1, int64_t a2, svfloat16_t a3) { svst1_vnum_f16(a0, a1, a2, a3); }
void t11(svbool_t a0, int16_t * a1, svuint32_t a2, svint32_t a3) { svst1h_scatter_u32offset_s32(a0, a1, a2, a3); }
void t12(svbool_t a0, int32_t * a1, svint32x2_t a2) { svst2_s32(a0, a1, a2); }
void t13(svbool_t a0, uint8_t * a1, svuint8x4_t a2) { svst4_u8(a0, a1, a2); }
void t14(svbool_t a0, int64_t * a1, svint64_t a2) { svstnt1_s64(a0, a1, a2); }
void t15(svbool_t a0, void const * a1) { svprfb(a0, a1, 1); }
void t16(svbool_t a0, void const * a1, int64_t a2) { svprfh_vnum(a0, a1, a2, 1); }
void t17(svbool_t a0, svuint64_t a1) { svprfd_gather_u64base(a0, a1, 1); }
svint32_t t18(svbool_t a0, int32_t const * a1) { return svld1rq_s32(a0, a1); }
svfloat32_t t19(svbool_t a0, float32_t const * a1) { return svld1ro_f32(a0, a1); }
void t20(svbool_t a0) { svwrffr(a0); }
svbool_t t21(svbool_t a0) { return svrdffr_z(a0); }
void t22() { svsetffr(); }
svuint32_t t23(svuint32_t a0, svint32_t a1) { return svadrb_u32base_s32offset(a0, a1); }
svint64_t t24(svbool_t a0, uint8_t const * a1, svuint64_t a2) { return svld1ub_gather_u64offset_s64(a0, a1, a2); }
svfloat32_t t25(svbool_t a0, svuint32_t a1, int64_t a2) { return svldff1_gather_u32base_index_f32(a0, a1, a2); }
