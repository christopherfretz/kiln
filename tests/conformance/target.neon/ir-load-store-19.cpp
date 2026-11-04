// [driver] The NEON load/store builtins lower as clang 19's
// `EmitAArch64BuiltinExpr`: `vld1[q]_v`/`vst1[q]_v` a plain vector
// `load`/`store` of the type-code's vector, `vld1[q]_lane_v` a scalar load
// and `insertelement`, `vst1[q]_lane_v` an `extractelement` and scalar
// store, `vld1[q]_dup_v` a load, `insertelement` into poison and a zero
// splat `shufflevector`, `vldrq_p128`/`vstrq_p128` an `i128` access; the
// alignment is the pointer argument's known alignment, the element's for
// a plain element pointer (spec/landed/codegen/WP-628.md
// "NEON load/store builtins"). WP-628.
//@ requires: clang<22
//@ requires: target-aarch64
//@ expect: ir
//@ args: -ftrivial-auto-var-init=uninitialized
#include <arm_neon.h>
uint8x8_t ld_u8(const uint8_t* p) { return vld1_u8(p); }
uint8x16_t ldq_u8(const uint8_t* p) { return vld1q_u8(p); }
int16x4_t ld_s16(const int16_t* p) { return vld1_s16(p); }
int16x8_t ldq_s16(const int16_t* p) { return vld1q_s16(p); }
float16x4_t ld_f16(const float16_t* p) { return vld1_f16(p); }
float16x8_t ldq_f16(const float16_t* p) { return vld1q_f16(p); }
int32x2_t ld_s32(const int32_t* p) { return vld1_s32(p); }
int32x4_t ldq_s32(const int32_t* p) { return vld1q_s32(p); }
float32x2_t ld_f32(const float32_t* p) { return vld1_f32(p); }
float32x4_t ldq_f32(const float32_t* p) { return vld1q_f32(p); }
uint64x1_t ld_u64(const uint64_t* p) { return vld1_u64(p); }
uint64x2_t ldq_u64(const uint64_t* p) { return vld1q_u64(p); }
poly64x1_t ld_p64(const poly64_t* p) { return vld1_p64(p); }
poly64x2_t ldq_p64(const poly64_t* p) { return vld1q_p64(p); }
float64x1_t ld_f64(const float64_t* p) { return vld1_f64(p); }
float64x2_t ldq_f64(const float64_t* p) { return vld1q_f64(p); }
void st_u8(uint8_t* p, uint8x8_t v) { vst1_u8(p, v); }
void stq_u8(uint8_t* p, uint8x16_t v) { vst1q_u8(p, v); }
void st_s16(int16_t* p, int16x4_t v) { vst1_s16(p, v); }
void stq_s16(int16_t* p, int16x8_t v) { vst1q_s16(p, v); }
void st_f16(float16_t* p, float16x4_t v) { vst1_f16(p, v); }
void stq_f16(float16_t* p, float16x8_t v) { vst1q_f16(p, v); }
void st_s32(int32_t* p, int32x2_t v) { vst1_s32(p, v); }
void stq_s32(int32_t* p, int32x4_t v) { vst1q_s32(p, v); }
void st_f32(float32_t* p, float32x2_t v) { vst1_f32(p, v); }
void stq_f32(float32_t* p, float32x4_t v) { vst1q_f32(p, v); }
void st_u64(uint64_t* p, uint64x1_t v) { vst1_u64(p, v); }
void stq_u64(uint64_t* p, uint64x2_t v) { vst1q_u64(p, v); }
void st_p64(poly64_t* p, poly64x1_t v) { vst1_p64(p, v); }
void stq_p64(poly64_t* p, poly64x2_t v) { vst1q_p64(p, v); }
void st_f64(float64_t* p, float64x1_t v) { vst1_f64(p, v); }
void stq_f64(float64_t* p, float64x2_t v) { vst1q_f64(p, v); }
uint8x8_t ldl_u8(const uint8_t* p, uint8x8_t v) { return vld1_lane_u8(p, v, 7); }
uint8x16_t ldlq_u8(const uint8_t* p, uint8x16_t v) { return vld1q_lane_u8(p, v, 15); }
int16x4_t ldl_s16(const int16_t* p, int16x4_t v) { return vld1_lane_s16(p, v, 3); }
int16x8_t ldlq_s16(const int16_t* p, int16x8_t v) { return vld1q_lane_s16(p, v, 7); }
float16x4_t ldl_f16(const float16_t* p, float16x4_t v) { return vld1_lane_f16(p, v, 3); }
float16x8_t ldlq_f16(const float16_t* p, float16x8_t v) { return vld1q_lane_f16(p, v, 7); }
int32x2_t ldl_s32(const int32_t* p, int32x2_t v) { return vld1_lane_s32(p, v, 1); }
int32x4_t ldlq_s32(const int32_t* p, int32x4_t v) { return vld1q_lane_s32(p, v, 3); }
float32x2_t ldl_f32(const float32_t* p, float32x2_t v) { return vld1_lane_f32(p, v, 1); }
float32x4_t ldlq_f32(const float32_t* p, float32x4_t v) { return vld1q_lane_f32(p, v, 3); }
uint64x1_t ldl_u64(const uint64_t* p, uint64x1_t v) { return vld1_lane_u64(p, v, 0); }
uint64x2_t ldlq_u64(const uint64_t* p, uint64x2_t v) { return vld1q_lane_u64(p, v, 1); }
poly64x1_t ldl_p64(const poly64_t* p, poly64x1_t v) { return vld1_lane_p64(p, v, 0); }
poly64x2_t ldlq_p64(const poly64_t* p, poly64x2_t v) { return vld1q_lane_p64(p, v, 1); }
float64x1_t ldl_f64(const float64_t* p, float64x1_t v) { return vld1_lane_f64(p, v, 0); }
float64x2_t ldlq_f64(const float64_t* p, float64x2_t v) { return vld1q_lane_f64(p, v, 1); }
void stl_u8(uint8_t* p, uint8x8_t v) { vst1_lane_u8(p, v, 7); }
void stlq_u8(uint8_t* p, uint8x16_t v) { vst1q_lane_u8(p, v, 15); }
void stl_s16(int16_t* p, int16x4_t v) { vst1_lane_s16(p, v, 3); }
void stlq_s16(int16_t* p, int16x8_t v) { vst1q_lane_s16(p, v, 7); }
void stl_f16(float16_t* p, float16x4_t v) { vst1_lane_f16(p, v, 3); }
void stlq_f16(float16_t* p, float16x8_t v) { vst1q_lane_f16(p, v, 7); }
void stl_s32(int32_t* p, int32x2_t v) { vst1_lane_s32(p, v, 1); }
void stlq_s32(int32_t* p, int32x4_t v) { vst1q_lane_s32(p, v, 3); }
void stl_f32(float32_t* p, float32x2_t v) { vst1_lane_f32(p, v, 1); }
void stlq_f32(float32_t* p, float32x4_t v) { vst1q_lane_f32(p, v, 3); }
void stl_u64(uint64_t* p, uint64x1_t v) { vst1_lane_u64(p, v, 0); }
void stlq_u64(uint64_t* p, uint64x2_t v) { vst1q_lane_u64(p, v, 1); }
void stl_p64(poly64_t* p, poly64x1_t v) { vst1_lane_p64(p, v, 0); }
void stlq_p64(poly64_t* p, poly64x2_t v) { vst1q_lane_p64(p, v, 1); }
void stl_f64(float64_t* p, float64x1_t v) { vst1_lane_f64(p, v, 0); }
void stlq_f64(float64_t* p, float64x2_t v) { vst1q_lane_f64(p, v, 1); }
uint8x8_t dup_u8(const uint8_t* p) { return vld1_dup_u8(p); }
uint8x16_t dupq_u8(const uint8_t* p) { return vld1q_dup_u8(p); }
int16x4_t dup_s16(const int16_t* p) { return vld1_dup_s16(p); }
int16x8_t dupq_s16(const int16_t* p) { return vld1q_dup_s16(p); }
float16x4_t dup_f16(const float16_t* p) { return vld1_dup_f16(p); }
float16x8_t dupq_f16(const float16_t* p) { return vld1q_dup_f16(p); }
int32x2_t dup_s32(const int32_t* p) { return vld1_dup_s32(p); }
int32x4_t dupq_s32(const int32_t* p) { return vld1q_dup_s32(p); }
float32x2_t dup_f32(const float32_t* p) { return vld1_dup_f32(p); }
float32x4_t dupq_f32(const float32_t* p) { return vld1q_dup_f32(p); }
uint64x1_t dup_u64(const uint64_t* p) { return vld1_dup_u64(p); }
uint64x2_t dupq_u64(const uint64_t* p) { return vld1q_dup_u64(p); }
poly64x1_t dup_p64(const poly64_t* p) { return vld1_dup_p64(p); }
poly64x2_t dupq_p64(const poly64_t* p) { return vld1q_dup_p64(p); }
float64x1_t dup_f64(const float64_t* p) { return vld1_dup_f64(p); }
float64x2_t dupq_f64(const float64_t* p) { return vld1q_dup_f64(p); }
struct alignas(16) Block { uint32_t w[8]; };
uint32x4_t ld_member(const Block* b) { return vld1q_u32(b->w + 4); }
poly128_t ldr(const poly128_t* p) { return vldrq_p128(p); }
void str(poly128_t* p, poly128_t v) { vstrq_p128(p, v); }
