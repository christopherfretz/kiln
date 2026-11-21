// [ACLE] SVE tuple intrinsics: svcreate, svget, svset, svundef.
//
// Each builtin is declared with __clang_arm_builtin_alias, as <arm_sve.h>
// and <arm_sme.h> declare it, and called once; clang's -O0 lowering
// (EmitAArch64SVEBuiltinExpr / EmitAArch64SMEBuiltinExpr) is the expected
// IR. spec/landed/codegen/WP-810.md "Builtin aliases" (WP-1739).
//@ requires: clang<22
//@ expect: ir
//@ args: --target=aarch64-unknown-linux-gnu -march=armv9.5-a+sve2p1+sme2p1+sve2-aes+sve2-sha3+sve2-sm4+sve2-bitperm+f32mm+f64mm+i8mm+bf16+b16b16+sme-f64f64+sme-i16i64+sme-f16f16+sme-b16b16+lut+faminmax+sme-fa64+sme-lutv2
typedef __SVBfloat16_t svbfloat16_t;
typedef __SVBool_t svbool_t;
typedef __SVFloat16_t svfloat16_t;
typedef __SVFloat32_t svfloat32_t;
typedef __SVFloat64_t svfloat64_t;
typedef __SVInt32_t svint32_t;
typedef __SVInt8_t svint8_t;
typedef __SVUint16_t svuint16_t;
typedef __SVUint64_t svuint64_t;
typedef __UINT64_TYPE__ uint64_t;
typedef __clang_svbfloat16x4_t svbfloat16x4_t;
typedef __clang_svboolx2_t svboolx2_t;
typedef __clang_svboolx4_t svboolx4_t;
typedef __clang_svfloat16x3_t svfloat16x3_t;
typedef __clang_svfloat32x3_t svfloat32x3_t;
typedef __clang_svfloat64x4_t svfloat64x4_t;
typedef __clang_svint32x2_t svint32x2_t;
typedef __clang_svint8x2_t svint8x2_t;
typedef __clang_svuint16x4_t svuint16x4_t;
typedef __clang_svuint64x3_t svuint64x3_t;
typedef __clang_svuint8x2_t svuint8x2_t;
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svcreate2_s8)))
svint8x2_t svcreate2_s8(svint8_t, svint8_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svcreate3_f32)))
svfloat32x3_t svcreate3_f32(svfloat32_t, svfloat32_t, svfloat32_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svcreate4_u16)))
svuint16x4_t svcreate4_u16(svuint16_t, svuint16_t, svuint16_t, svuint16_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svget2_s32)))
svint32_t svget2_s32(svint32x2_t, uint64_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svget3_f16)))
svfloat16_t svget3_f16(svfloat16x3_t, uint64_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svget4_bf16)))
svbfloat16_t svget4_bf16(svbfloat16x4_t, uint64_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svset2_s8)))
svint8x2_t svset2_s8(svint8x2_t, uint64_t, svint8_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svset3_u64)))
svuint64x3_t svset3_u64(svuint64x3_t, uint64_t, svuint64_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svset4_f64)))
svfloat64x4_t svset4_f64(svfloat64x4_t, uint64_t, svfloat64_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svundef_s32)))
svint32_t svundef_s32();
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svundef2_u8)))
svuint8x2_t svundef2_u8();
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svundef3_f16)))
svfloat16x3_t svundef3_f16();
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svundef4_bf16)))
svbfloat16x4_t svundef4_bf16();
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svcreate2_b)))
svboolx2_t svcreate2_b(svbool_t, svbool_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svget2_b)))
svbool_t svget2_b(svboolx2_t, uint64_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svset4_b)))
svboolx4_t svset4_b(svboolx4_t, uint64_t, svbool_t);
svint8x2_t t0(svint8_t a0, svint8_t a1) { return svcreate2_s8(a0, a1); }
svfloat32x3_t t1(svfloat32_t a0, svfloat32_t a1, svfloat32_t a2) { return svcreate3_f32(a0, a1, a2); }
svuint16x4_t t2(svuint16_t a0, svuint16_t a1, svuint16_t a2, svuint16_t a3) { return svcreate4_u16(a0, a1, a2, a3); }
svint32_t t3(svint32x2_t a0) { return svget2_s32(a0, 0); }
svfloat16_t t4(svfloat16x3_t a0) { return svget3_f16(a0, 0); }
svbfloat16_t t5(svbfloat16x4_t a0) { return svget4_bf16(a0, 0); }
svint8x2_t t6(svint8x2_t a0, svint8_t a2) { return svset2_s8(a0, 0, a2); }
svuint64x3_t t7(svuint64x3_t a0, svuint64_t a2) { return svset3_u64(a0, 0, a2); }
svfloat64x4_t t8(svfloat64x4_t a0, svfloat64_t a2) { return svset4_f64(a0, 0, a2); }
svint32_t t9() { return svundef_s32(); }
svuint8x2_t t10() { return svundef2_u8(); }
svfloat16x3_t t11() { return svundef3_f16(); }
svbfloat16x4_t t12() { return svundef4_bf16(); }
svboolx2_t t13(svbool_t a0, svbool_t a1) { return svcreate2_b(a0, a1); }
svbool_t t14(svboolx2_t a0) { return svget2_b(a0, 0); }
svboolx4_t t15(svboolx4_t a0, svbool_t a2) { return svset4_b(a0, 0, a2); }
