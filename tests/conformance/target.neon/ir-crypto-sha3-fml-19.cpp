// [driver] SHA3 (`bcax`, `eor3`, `rax1`, `xar`), SHA-512 and SM3/SM4
// (declared `nounwind memory(none)`, `sm3tt*` and `xar` with an `immarg`),
// and the `half` widening multiply-accumulates `vfmlal`/`vfmlsl` as clang 19
// lowers them (spec/codegen.md "AArch64 NEON builtins"; from a census of
// arm_neon.h, scripts/neon-intrinsic-census.py). WP-1421.
//@ requires: clang<22
//@ requires: target-aarch64
//@ expect: ir
//@ args: -ftrivial-auto-var-init=uninitialized
#include <arm_neon.h>
__attribute__((target("sha3,neon"))) auto f_vbcaxq_s16(int16x8_t a0, int16x8_t a1, int16x8_t a2) { return vbcaxq_s16(a0, a1, a2); }
__attribute__((target("sha3,neon"))) auto f_vbcaxq_s32(int32x4_t a0, int32x4_t a1, int32x4_t a2) { return vbcaxq_s32(a0, a1, a2); }
__attribute__((target("sha3,neon"))) auto f_vbcaxq_s8(int8x16_t a0, int8x16_t a1, int8x16_t a2) { return vbcaxq_s8(a0, a1, a2); }
__attribute__((target("sha3,neon"))) auto f_vbcaxq_u32(uint32x4_t a0, uint32x4_t a1, uint32x4_t a2) { return vbcaxq_u32(a0, a1, a2); }
__attribute__((target("sha3,neon"))) auto f_vbcaxq_u8(uint8x16_t a0, uint8x16_t a1, uint8x16_t a2) { return vbcaxq_u8(a0, a1, a2); }
__attribute__((target("sha3,neon"))) auto f_veor3q_s32(int32x4_t a0, int32x4_t a1, int32x4_t a2) { return veor3q_s32(a0, a1, a2); }
__attribute__((target("sha3,neon"))) auto f_veor3q_s8(int8x16_t a0, int8x16_t a1, int8x16_t a2) { return veor3q_s8(a0, a1, a2); }
__attribute__((target("sha3,neon"))) auto f_veor3q_u32(uint32x4_t a0, uint32x4_t a1, uint32x4_t a2) { return veor3q_u32(a0, a1, a2); }
__attribute__((target("sha3,neon"))) auto f_veor3q_u8(uint8x16_t a0, uint8x16_t a1, uint8x16_t a2) { return veor3q_u8(a0, a1, a2); }
__attribute__((target("sha3,neon"))) auto f_vsha512h2q_u64(uint64x2_t a0, uint64x2_t a1, uint64x2_t a2) { return vsha512h2q_u64(a0, a1, a2); }
__attribute__((target("sha3,neon"))) auto f_vsha512su0q_u64(uint64x2_t a0, uint64x2_t a1) { return vsha512su0q_u64(a0, a1); }
__attribute__((target("sm4,neon"))) auto f_vsm3partw1q_u32(uint32x4_t a0, uint32x4_t a1, uint32x4_t a2) { return vsm3partw1q_u32(a0, a1, a2); }
__attribute__((target("sm4,neon"))) auto f_vsm3ss1q_u32(uint32x4_t a0, uint32x4_t a1, uint32x4_t a2) { return vsm3ss1q_u32(a0, a1, a2); }
__attribute__((target("sm4,neon"))) auto f_vsm3tt1bq_u32(uint32x4_t a0, uint32x4_t a1, uint32x4_t a2) { return vsm3tt1bq_u32(a0, a1, a2, 1); }
__attribute__((target("sm4,neon"))) auto f_vsm3tt2bq_u32(uint32x4_t a0, uint32x4_t a1, uint32x4_t a2) { return vsm3tt2bq_u32(a0, a1, a2, 1); }
__attribute__((target("sm4,neon"))) auto f_vsm4eq_u32(uint32x4_t a0, uint32x4_t a1) { return vsm4eq_u32(a0, a1); }
__attribute__((target("fp16fml,neon"))) auto f_vfmlal_high_f16(float32x2_t a0, float16x4_t a1, float16x4_t a2) { return vfmlal_high_f16(a0, a1, a2); }
__attribute__((target("fp16fml"))) auto f_vfmlal_lane_low_f16(float32x2_t a0, float16x4_t a1, float16x4_t a2) { return vfmlal_lane_low_f16(a0, a1, a2, 1); }
__attribute__((target("fp16fml,neon"))) auto f_vfmlalq_high_f16(float32x4_t a0, float16x8_t a1, float16x8_t a2) { return vfmlalq_high_f16(a0, a1, a2); }
__attribute__((target("fp16fml"))) auto f_vfmlalq_lane_low_f16(float32x4_t a0, float16x8_t a1, float16x4_t a2) { return vfmlalq_lane_low_f16(a0, a1, a2, 1); }
__attribute__((target("fp16fml,neon"))) auto f_vfmlsl_high_f16(float32x2_t a0, float16x4_t a1, float16x4_t a2) { return vfmlsl_high_f16(a0, a1, a2); }
__attribute__((target("fp16fml"))) auto f_vfmlsl_lane_low_f16(float32x2_t a0, float16x4_t a1, float16x4_t a2) { return vfmlsl_lane_low_f16(a0, a1, a2, 1); }
__attribute__((target("fp16fml,neon"))) auto f_vfmlslq_high_f16(float32x4_t a0, float16x8_t a1, float16x8_t a2) { return vfmlslq_high_f16(a0, a1, a2); }
__attribute__((target("fp16fml"))) auto f_vfmlslq_lane_low_f16(float32x4_t a0, float16x8_t a1, float16x4_t a2) { return vfmlslq_lane_low_f16(a0, a1, a2, 1); }
__attribute__((target("sha3,neon"))) auto f_vrax1q_u64(uint64x2_t a0, uint64x2_t a1) { return vrax1q_u64(a0, a1); }
__attribute__((target("sha3,neon"))) auto f_vxarq_u64(uint64x2_t a0, uint64x2_t a1) { return vxarq_u64(a0, a1, 1); }
