// [ACLE] An SVE intrinsic call nested in another's operand declares its
// LLVM intrinsics first, as clang: clang emits every operand
// (`GetAArch64SVEProcessedOperands`) before it creates the outer
// builtin's intrinsics, so `svadd_s32_z(svptrue_b32(), a, b)` declares
// `ptrue` and `convert.to.svbool` before `convert.from.svbool` and `add`,
// and the attribute groups are numbered in that order. Found by WP-1794
// (a real <arm_sve.h> call); spec/landed/codegen/WP-810.md "Builtin
// aliases". No header is included under the pinned --target.
//@ expect: ir
//@ args: --target=aarch64-unknown-linux-gnu -march=armv8-a+sve -mno-outline-atomics
typedef __SVBool_t svbool_t;
typedef __SVInt32_t svint32_t;
typedef __SVFloat32_t svfloat32_t;
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svptrue_b32)))
svbool_t svptrue_b32();
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svadd_s32_z)))
svint32_t svadd_s32_z(svbool_t, svint32_t, svint32_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svdup_n_f32)))
svfloat32_t svdup_n_f32(float);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svmul_f32_x)))
svfloat32_t svmul_f32_x(svbool_t, svfloat32_t, svfloat32_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svcntw)))
unsigned long svcntw();
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svdup_n_s32)))
svint32_t svdup_n_s32(int);
svint32_t add(svint32_t a, svint32_t b) { return svadd_s32_z(svptrue_b32(), a, b); }
svfloat32_t mul(svfloat32_t a) { return svmul_f32_x(svptrue_b32(), a, svdup_n_f32(2.0f)); }
svint32_t nested(svint32_t a) {
  return svadd_s32_z(svptrue_b32(), svdup_n_s32((int)svcntw()), a);
}
