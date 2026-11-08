// [ACLE] SVE intrinsics from <arm_sve.h> (ACLE "SVE language extensions
// and intrinsics"; clang AttrDocs "__clang_arm_builtin_alias"). Each
// intrinsic is a declaration aliased to a `__builtin_sve_*` builtin and
// lowers to its `llvm.aarch64.sve.*` intrinsic: predicates through
// `convert.{from,to}.svbool`, `_n` operands splatted, `_z` operand 1
// selected against zero, `_x` through the `.u` intrinsic.
// spec/landed/codegen/WP-810.md (WP-810).
// Host-restricted (no pinned `--target`): on an x86_64 host clang has no
// AArch64 sysroot and leaves `+outline-atomics` out of the features.
//@ requires: clang>=22
//@ requires: target-aarch64
//@ expect: ir
//@ std: c++20
//@ args: -march=armv8-a+sve
#include <arm_sve.h>
uint64_t f5() { return svcntw(); }
uint64_t f8() { return svcntd_pat(SV_POW2); }
svbool_t f10() { return svptrue_pat_b8(SV_VL3); }
svbool_t f29() { return svptrue_b32(); }
svbool_t f31(int32_t a, int32_t b) { return svwhilelt_b32_s32(a, b); }
svbool_t f48(uint64_t a, uint64_t b) { return svwhilele_b64_u64(a, b); }
svint32_t f173(svbool_t pg, const int32_t *p) { return svld1_s32(pg, p); }
void ss32(svbool_t pg, int32_t *p, svint32_t v) { svst1_s32(pg, p, v); }
svint32_t f174(int32_t x) { return svdup_n_s32(x); }
svint32_t f175(svbool_t pg, svint32_t a, svint32_t b) { return svadd_s32_m(pg, a, b); }
svint32_t f176(svbool_t pg, svint32_t a, int32_t b) { return svadd_n_s32_m(pg, a, b); }
svint32_t f177(svbool_t pg, svint32_t a, svint32_t b) { return svadd_s32_x(pg, a, b); }
svint32_t f178(svbool_t pg, svint32_t a, int32_t b) { return svadd_n_s32_x(pg, a, b); }
int64_t f235(svbool_t pg, svint32_t v) { return svaddv_s32(pg, v); }
void su8(svbool_t pg, uint8_t *p, svuint8_t v) { svst1_u8(pg, p, v); }
svuint8_t f322(svbool_t pg, svuint8_t a, uint8_t b) { return svsub_n_u8_z(pg, a, b); }
uint16_t f428(svbool_t pg, svuint16_t v) { return svmaxv_u16(pg, v); }
svfloat32_t f617(float32_t x) { return svdup_n_f32(x); }
svfloat32_t f634(svbool_t pg, svfloat32_t a, svfloat32_t b) { return svmul_f32_z(pg, a, b); }
svfloat32_t f648(svbool_t pg, svfloat32_t a, svfloat32_t b, svfloat32_t c) { return svmla_f32_m(pg, a, b, c); }
float32_t f660(svbool_t pg, svfloat32_t v) { return svaddv_f32(pg, v); }
svfloat64_t f663(svbool_t pg, const float64_t *p) { return svld1_f64(pg, p); }
