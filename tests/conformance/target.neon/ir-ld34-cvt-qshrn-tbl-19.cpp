// [driver] `vld3`/`vld4`/`vst3`/`vst4` lower to `ld3`/`ld4`/`st3`/`st4`, `vcvt{n,a,m,p}` to
// `fcvt{n,a,m,p}{s,u}`, `vhadd`/`vrhadd`/`vhsub` to `{s,u}hadd`/`{s,u}rhadd`/`{s,u}hsub`,
// `vq(r)shr(u)n_n` to `{s,u}q(r)shrn`/`sq(r)shrun`, `vqshl_n` to `{s,u}qshl` by a splat,
// `vrshr_n` to `{s,u}rshl` by a negated splat and `vtbl1`/`vtbl2` to a widening shuffle plus
// `tbl1`, as clang 19's `EmitAArch64BuiltinExpr` (OpenCV's carotene HAL; WP-803).
//@ requires: clang<22
//@ requires: target-aarch64
//@ expect: ir
//@ args: -ftrivial-auto-var-init=uninitialized
#include <arm_neon.h>
auto f_vld3_u8(const unsigned char *p) { return vld3_u8(p); }
auto f_vld3q_f32(const float *p) { return vld3q_f32(p); }
auto f_vld4q_u8(const unsigned char *p) { return vld4q_u8(p); }
auto f_vld4_s16(const short *p) { return vld4_s16(p); }
void f_vst3_s16(short *p, int16x4x3_t v) { vst3_s16(p, v); }
void f_vst3q_u8(unsigned char *p, uint8x16x3_t v) { vst3q_u8(p, v); }
void f_vst4q_u32(unsigned *p, uint32x4x4_t v) { vst4q_u32(p, v); }
void f_vst4_u8(unsigned char *p, uint8x8x4_t v) { vst4_u8(p, v); }
auto f_vcvtnq_s32_f32(float32x4_t a) { return vcvtnq_s32_f32(a); }
auto f_vcvta_s32_f32(float32x2_t a) { return vcvta_s32_f32(a); }
auto f_vcvtmq_u32_f32(float32x4_t a) { return vcvtmq_u32_f32(a); }
auto f_vcvtpq_s64_f64(float64x2_t a) { return vcvtpq_s64_f64(a); }
auto f_vhadd_s8(int8x8_t a, int8x8_t b) { return vhadd_s8(a, b); }
auto f_vhaddq_u16(uint16x8_t a, uint16x8_t b) { return vhaddq_u16(a, b); }
auto f_vrhaddq_u8(uint8x16_t a, uint8x16_t b) { return vrhaddq_u8(a, b); }
auto f_vhsub_s16(int16x4_t a, int16x4_t b) { return vhsub_s16(a, b); }
auto f_vqrshrn_n_s16(int16x8_t a) { return vqrshrn_n_s16(a, 3); }
auto f_vqshrn_n_u32(uint32x4_t a) { return vqshrn_n_u32(a, 2); }
auto f_vqrshrun_n_s16(int16x8_t a) { return vqrshrun_n_s16(a, 3); }
auto f_vqshrun_n_s32(int32x4_t a) { return vqshrun_n_s32(a, 4); }
auto f_vqshl_n_s8(int8x8_t a) { return vqshl_n_s8(a, 3); }
auto f_vqshlq_n_u16(uint16x8_t a) { return vqshlq_n_u16(a, 2); }
auto f_vrshr_n_s16(int16x4_t a) { return vrshr_n_s16(a, 2); }
auto f_vrshrq_n_u8(uint8x16_t a) { return vrshrq_n_u8(a, 1); }
auto f_vtbl1_u8(uint8x8_t a, uint8x8_t b) { return vtbl1_u8(a, b); }
auto f_vtbl2_s8(int8x8x2_t a, int8x8_t b) { return vtbl2_s8(a, b); }
