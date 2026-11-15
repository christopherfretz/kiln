// [driver] The scalar (`b`, `h`, `s`, `d`) forms: a `b`/`h` operand inserted
// at lane 0 of a 64-bit `poison` vector and lane 0 of the result taken, an
// `s`/`d` one a scalar call; `vqdmlal{h,s}`, `vaddd`, `vnegd`,
// `vshld_n`/`vshrd_n`, `vrsrad_n`, `vfmah`, the `half` arithmetic and the
// `float` reductions as clang 19 lowers them (spec/codegen.md "AArch64 NEON
// builtins"; from a census of arm_neon.h, scripts/neon-intrinsic-census.py).
// WP-1421.
//@ requires: clang<22
//@ requires: target-aarch64
//@ expect: ir
//@ args: -ftrivial-auto-var-init=uninitialized
#include <arm_fp16.h>
#include <arm_neon.h>
__attribute__((target("neon"))) auto f_vqabsb_s8(int8_t a0) { return vqabsb_s8(a0); }
__attribute__((target("neon"))) auto f_vqaddd_s64(int64_t a0, int64_t a1) { return vqaddd_s64(a0, a1); }
__attribute__((target("neon"))) auto f_vqdmlalh_s16(int32_t a0, int16_t a1, int16_t a2) { return vqdmlalh_s16(a0, a1, a2); }
auto f_vqdmullh_lane_s16(int16_t a0, int16x4_t a1) { return vqdmullh_lane_s16(a0, a1, 1); }
__attribute__((target("neon"))) auto f_vqmovns_s32(int32_t a0) { return vqmovns_s32(a0); }
__attribute__((target("neon"))) auto f_vqnegd_s64(int64_t a0) { return vqnegd_s64(a0); }
__attribute__((target("v8.1a"))) auto f_vqrdmlahs_lane_s32(int32_t a0, int32_t a1, int32x2_t a2) { return vqrdmlahs_lane_s32(a0, a1, a2, 1); }
auto f_vqrdmulhs_lane_s32(int32_t a0, int32x2_t a1) { return vqrdmulhs_lane_s32(a0, a1, 1); }
__attribute__((target("neon"))) auto f_vqrshlh_u16(uint16_t a0, int16_t a1) { return vqrshlh_u16(a0, a1); }
auto f_vqrshrnh_n_u16(uint16_t a0) { return vqrshrnh_n_u16(a0, 1); }
auto f_vqshlb_n_s8(int8_t a0) { return vqshlb_n_s8(a0, 1); }
__attribute__((target("neon"))) auto f_vqshld_s64(int64_t a0, int64_t a1) { return vqshld_s64(a0, a1); }
auto f_vqshls_n_s32(int32_t a0) { return vqshls_n_s32(a0, 1); }
auto f_vqshluh_n_s16(int16_t a0) { return vqshluh_n_s16(a0, 1); }
auto f_vqshrns_n_s32(int32_t a0) { return vqshrns_n_s32(a0, 1); }
__attribute__((target("neon"))) auto f_vqsubb_u8(uint8_t a0, uint8_t a1) { return vqsubb_u8(a0, a1); }
__attribute__((target("neon"))) auto f_vabdd_f64(float64_t a0, float64_t a1) { return vabdd_f64(a0, a1); }
__attribute__((target("fullfp16"))) auto f_vabsh_f16(float16_t a0) { return vabsh_f16(a0); }
__attribute__((target("fullfp16"))) auto f_vdivh_f16(float16_t a0, float16_t a1) { return vdivh_f16(a0, a1); }
__attribute__((target("fullfp16"))) auto f_vmaxnmh_f16(float16_t a0, float16_t a1) { return vmaxnmh_f16(a0, a1); }
auto f_vmulx_lane_f64(float64x1_t a0, float64x1_t a1) { return vmulx_lane_f64(a0, a1, 0); }
__attribute__((target("fullfp16"))) auto f_vnegh_f16(float16_t a0) { return vnegh_f16(a0); }
__attribute__((target("neon"))) auto f_vpadds_f32(float32x2_t a0) { return vpadds_f32(a0); }
__attribute__((target("neon"))) auto f_vpmaxs_f32(float32x2_t a0) { return vpmaxs_f32(a0); }
__attribute__((target("neon"))) auto f_vpmins_f32(float32x2_t a0) { return vpmins_f32(a0); }
__attribute__((target("neon"))) auto f_vrecpsd_f64(float64_t a0, float64_t a1) { return vrecpsd_f64(a0, a1); }
__attribute__((target("fullfp16"))) auto f_vrecpxh_f16(float16_t a0) { return vrecpxh_f16(a0); }
__attribute__((target("neon"))) auto f_vrsqrted_f64(float64_t a0) { return vrsqrted_f64(a0); }
__attribute__((target("fullfp16"))) auto f_vrsqrtsh_f16(float16_t a0, float16_t a1) { return vrsqrtsh_f16(a0, a1); }
__attribute__((target("neon"))) auto f_vsqaddb_u8(uint8_t a0, int8_t a1) { return vsqaddb_u8(a0, a1); }
__attribute__((target("fullfp16"))) auto f_vsqrth_f16(float16_t a0) { return vsqrth_f16(a0); }
__attribute__((target("neon"))) auto f_vuqaddb_s8(int8_t a0, uint8_t a1) { return vuqaddb_s8(a0, a1); }
__attribute__((target("neon"))) auto f_vaddd_s64(int64_t a0, int64_t a1) { return vaddd_s64(a0, a1); }
__attribute__((target("neon"))) auto f_vnegd_s64(int64_t a0) { return vnegd_s64(a0); }
auto f_vshrd_n_u64(uint64_t a0) { return vshrd_n_u64(a0, 1); }
auto f_vshrd_n_s64(int64_t a0) { return vshrd_n_s64(a0, 1); }
auto f_vrsrad_n_s64(int64_t a0, int64_t a1) { return vrsrad_n_s64(a0, a1, 1); }
__attribute__((target("fullfp16"))) auto f_vaddh_f16(float16_t a0, float16_t a1) { return vaddh_f16(a0, a1); }
