// [driver] The multi-register, replicating and by-lane loads and stores:
// `vld1_x{2,3,4}` (`ld1xK`), `vldK_dup` (`ldKr`), `vldK_lane` (`ldKlane` of
// the vectors, the lane as `i64`, the pointer), `vst1_x{2,3,4}` (`st1xK`),
// `vstK_lane` (`stKlane`), and `vldap1_lane`/`vstl1_lane` as an acquire load
// / release store of the lane as clang 19 lowers them (spec/codegen.md
// "AArch64 NEON builtins"; from a census of arm_neon.h,
// scripts/neon-intrinsic-census.py). WP-1421.
//@ requires: clang<22
//@ requires: target-aarch64
//@ expect: ir
//@ args: -ftrivial-auto-var-init=uninitialized
#include <arm_neon.h>
auto f_vld1_f16_x2(const float16_t *a0) { return vld1_f16_x2(a0); }
auto f_vld1_f16_x3(const float16_t *a0) { return vld1_f16_x3(a0); }
auto f_vld1q_f16_x2(const float16_t *a0) { return vld1q_f16_x2(a0); }
auto f_vld1q_f16_x4(const float16_t *a0) { return vld1q_f16_x4(a0); }
auto f_vld2_dup_f16(const float16_t *a0) { return vld2_dup_f16(a0); }
auto f_vld2_lane_f16(const float16_t *a0, float16x4x2_t a1) { return vld2_lane_f16(a0, a1, 1); }
auto f_vld2q_dup_f16(const float16_t *a0) { return vld2q_dup_f16(a0); }
auto f_vld2q_lane_f16(const float16_t *a0, float16x8x2_t a1) { return vld2q_lane_f16(a0, a1, 1); }
auto f_vld3_dup_f16(const float16_t *a0) { return vld3_dup_f16(a0); }
__attribute__((target("bf16,neon"))) auto f_vld3_lane_bf16(const bfloat16_t *a0, bfloat16x4x3_t a1) { return vld3_lane_bf16(a0, a1, 1); }
__attribute__((target("bf16,neon"))) auto f_vld3q_dup_bf16(const bfloat16_t *a0) { return vld3q_dup_bf16(a0); }
__attribute__((target("bf16,neon"))) auto f_vld3q_lane_bf16(const bfloat16_t *a0, bfloat16x8x3_t a1) { return vld3q_lane_bf16(a0, a1, 1); }
__attribute__((target("bf16,neon"))) auto f_vld4_dup_bf16(const bfloat16_t *a0) { return vld4_dup_bf16(a0); }
__attribute__((target("bf16,neon"))) auto f_vld4_lane_bf16(const bfloat16_t *a0, bfloat16x4x4_t a1) { return vld4_lane_bf16(a0, a1, 1); }
__attribute__((target("bf16,neon"))) auto f_vld4q_dup_bf16(const bfloat16_t *a0) { return vld4q_dup_bf16(a0); }
__attribute__((target("bf16,neon"))) auto f_vld4q_lane_bf16(const bfloat16_t *a0, bfloat16x8x4_t a1) { return vld4q_lane_bf16(a0, a1, 1); }
auto f_vst1_f16_x2(float16_t *a0, float16x4x2_t a1) { return vst1_f16_x2(a0, a1); }
auto f_vst1_f16_x3(float16_t *a0, float16x4x3_t a1) { return vst1_f16_x3(a0, a1); }
auto f_vst1_f16_x4(float16_t *a0, float16x4x4_t a1) { return vst1_f16_x4(a0, a1); }
auto f_vst1q_f16_x2(float16_t *a0, float16x8x2_t a1) { return vst1q_f16_x2(a0, a1); }
auto f_vst1q_f16_x3(float16_t *a0, float16x8x3_t a1) { return vst1q_f16_x3(a0, a1); }
auto f_vst1q_f16_x4(float16_t *a0, float16x8x4_t a1) { return vst1q_f16_x4(a0, a1); }
__attribute__((target("bf16,neon"))) auto f_vst2_lane_bf16(bfloat16_t *a0, bfloat16x4x2_t a1) { return vst2_lane_bf16(a0, a1, 1); }
auto f_vst2_lane_f16(float16_t *a0, float16x4x2_t a1) { return vst2_lane_f16(a0, a1, 1); }
auto f_vst2q_lane_f16(float16_t *a0, float16x8x2_t a1) { return vst2q_lane_f16(a0, a1, 1); }
__attribute__((target("bf16,neon"))) auto f_vst3_lane_bf16(bfloat16_t *a0, bfloat16x4x3_t a1) { return vst3_lane_bf16(a0, a1, 1); }
auto f_vst3_lane_f16(float16_t *a0, float16x4x3_t a1) { return vst3_lane_f16(a0, a1, 1); }
__attribute__((target("bf16,neon"))) auto f_vst3q_lane_bf16(bfloat16_t *a0, bfloat16x8x3_t a1) { return vst3q_lane_bf16(a0, a1, 1); }
auto f_vst3q_lane_f16(float16_t *a0, float16x8x3_t a1) { return vst3q_lane_f16(a0, a1, 1); }
__attribute__((target("bf16,neon"))) auto f_vst4_lane_bf16(bfloat16_t *a0, bfloat16x4x4_t a1) { return vst4_lane_bf16(a0, a1, 1); }
auto f_vst4_lane_f16(float16_t *a0, float16x4x4_t a1) { return vst4_lane_f16(a0, a1, 1); }
__attribute__((target("bf16,neon"))) auto f_vst4q_lane_bf16(bfloat16_t *a0, bfloat16x8x4_t a1) { return vst4q_lane_bf16(a0, a1, 1); }
__attribute__((target("bf16,neon"))) auto f_vld1_bf16_x2(const bfloat16_t *a0) { return vld1_bf16_x2(a0); }
__attribute__((target("bf16,neon"))) auto f_vld1_bf16_x3(const bfloat16_t *a0) { return vld1_bf16_x3(a0); }
__attribute__((target("bf16,neon"))) auto f_vld1q_bf16_x2(const bfloat16_t *a0) { return vld1q_bf16_x2(a0); }
__attribute__((target("bf16,neon"))) auto f_vld1q_bf16_x4(const bfloat16_t *a0) { return vld1q_bf16_x4(a0); }
__attribute__((target("rcpc3,neon"))) auto f_vldap1_lane_p64(const poly64_t *a0, poly64x1_t a1) { return vldap1_lane_p64(a0, a1, 0); }
__attribute__((target("rcpc3,neon"))) auto f_vldap1_lane_s64(const int64_t *a0, int64x1_t a1) { return vldap1_lane_s64(a0, a1, 0); }
__attribute__((target("rcpc3,neon"))) auto f_vldap1q_lane_f64(const float64_t *a0, float64x2_t a1) { return vldap1q_lane_f64(a0, a1, 1); }
__attribute__((target("rcpc3,neon"))) auto f_vldap1q_lane_s64(const int64_t *a0, int64x2_t a1) { return vldap1q_lane_s64(a0, a1, 1); }
__attribute__((target("bf16,neon"))) auto f_vst1_bf16_x2(bfloat16_t *a0, bfloat16x4x2_t a1) { return vst1_bf16_x2(a0, a1); }
__attribute__((target("bf16,neon"))) auto f_vst1_bf16_x3(bfloat16_t *a0, bfloat16x4x3_t a1) { return vst1_bf16_x3(a0, a1); }
__attribute__((target("bf16,neon"))) auto f_vst1q_bf16_x2(bfloat16_t *a0, bfloat16x8x2_t a1) { return vst1q_bf16_x2(a0, a1); }
__attribute__((target("bf16,neon"))) auto f_vst1q_bf16_x4(bfloat16_t *a0, bfloat16x8x4_t a1) { return vst1q_bf16_x4(a0, a1); }
__attribute__((target("rcpc3,neon"))) auto f_vstl1_lane_p64(poly64_t *a0, poly64x1_t a1) { return vstl1_lane_p64(a0, a1, 0); }
__attribute__((target("rcpc3,neon"))) auto f_vstl1_lane_s64(int64_t *a0, int64x1_t a1) { return vstl1_lane_s64(a0, a1, 0); }
__attribute__((target("rcpc3,neon"))) auto f_vstl1q_lane_f64(float64_t *a0, float64x2_t a1) { return vstl1q_lane_f64(a0, a1, 1); }
__attribute__((target("rcpc3,neon"))) auto f_vstl1q_lane_s64(int64_t *a0, int64x2_t a1) { return vstl1q_lane_s64(a0, a1, 1); }
auto f_vld1_u8_x2(const uint8_t *a0) { return vld1_u8_x2(a0); }
auto f_vst1q_f32_x4(float32_t *a0, float32x4x4_t a1) { return vst1q_f32_x4(a0, a1); }
