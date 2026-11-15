// [driver] The remaining vector builtins: `vadd` of polynomials (`xor`),
// `v{add,sub}hn`, `vr{add,sub}hn`, `vrsra_n`, `vcls`, `vrbit`,
// `vsqadd`/`vuqadd`, `vmulx`, `vpmaxnm`/`vpminnm`, `vcadd`,
// `vqdmlal`/`vqdmlsl`, `vqrdmlah`, `vqshlu_n`, `vrndi`/`vrndx`,
// `vrnd32`/`vrnd64`, and the by-lane forms (`sq[r]dmulh.lane[q]`, `vmul_lane`
// of `double`, `vfma{h,s,d}_lane`) as clang 19 lowers them (spec/codegen.md
// "AArch64 NEON builtins"; from a census of arm_neon.h,
// scripts/neon-intrinsic-census.py). WP-1421.
//@ requires: clang<22
//@ requires: target-aarch64
//@ expect: ir
//@ args: -ftrivial-auto-var-init=uninitialized
#include <arm_fp16.h>
#include <arm_neon.h>
__attribute__((target("neon"))) auto f_vadd_p16(poly16x4_t a0, poly16x4_t a1) { return vadd_p16(a0, a1); }
__attribute__((target("v8.3a,fullfp16,neon"))) auto f_vcadd_rot270_f16(float16x4_t a0, float16x4_t a1) { return vcadd_rot270_f16(a0, a1); }
__attribute__((target("v8.3a,fullfp16,neon"))) auto f_vcaddq_rot270_f16(float16x8_t a0, float16x8_t a1) { return vcaddq_rot270_f16(a0, a1); }
__attribute__((target("v8.3a,neon"))) auto f_vcaddq_rot90_f64(float64x2_t a0, float64x2_t a1) { return vcaddq_rot90_f64(a0, a1); }
__attribute__((target("neon"))) auto f_vmaxnmv_f32(float32x2_t a0) { return vmaxnmv_f32(a0); }
__attribute__((target("neon"))) auto f_vminnmv_f32(float32x2_t a0) { return vminnmv_f32(a0); }
__attribute__((target("i8mm,neon"))) auto f_vmmlaq_s32(int32x4_t a0, int8x16_t a1, int8x16_t a2) { return vmmlaq_s32(a0, a1, a2); }
__attribute__((target("fullfp16,neon"))) auto f_vmulx_f16(float16x4_t a0, float16x4_t a1) { return vmulx_f16(a0, a1); }
__attribute__((target("fullfp16,neon"))) auto f_vpmaxnm_f16(float16x4_t a0, float16x4_t a1) { return vpmaxnm_f16(a0, a1); }
__attribute__((target("fullfp16,neon"))) auto f_vpminnm_f16(float16x4_t a0, float16x4_t a1) { return vpminnm_f16(a0, a1); }
__attribute__((target("neon"))) auto f_vrbit_p8(poly8x8_t a0) { return vrbit_p8(a0); }
auto f_vrsra_n_s16(int16x4_t a0, int16x4_t a1) { return vrsra_n_s16(a0, a1, 1); }
auto f_vshld_n_s64(int64_t a0) { return vshld_n_s64(a0, 1); }
auto f_vslid_n_s64(int64_t a0, int64_t a1) { return vslid_n_s64(a0, a1, 1); }
auto f_vsrad_n_u64(uint64_t a0, uint64_t a1) { return vsrad_n_u64(a0, a1, 1); }
__attribute__((target("neon"))) auto f_vuqadd_s16(int16x4_t a0, uint16x4_t a1) { return vuqadd_s16(a0, a1); }
auto f_vqdmlal_high_lane_s16(int32x4_t a0, int16x8_t a1, int16x4_t a2) { return vqdmlal_high_lane_s16(a0, a1, a2, 1); }
auto f_vqdmlsl_high_lane_s16(int32x4_t a0, int16x8_t a1, int16x4_t a2) { return vqdmlsl_high_lane_s16(a0, a1, a2, 1); }
auto f_vqdmull_high_lane_s16(int16x8_t a0, int16x4_t a1) { return vqdmull_high_lane_s16(a0, a1, 1); }
__attribute__((target("v8.1a"))) auto f_vqrdmlahq_lane_s16(int16x8_t a0, int16x8_t a1, int16x4_t a2) { return vqrdmlahq_lane_s16(a0, a1, a2, 1); }
__attribute__((target("v8.1a"))) auto f_vqrdmlahq_lane_s32(int32x4_t a0, int32x4_t a1, int32x2_t a2) { return vqrdmlahq_lane_s32(a0, a1, a2, 1); }
__attribute__((target("v8.1a"))) auto f_vqrdmlshq_lane_s16(int16x8_t a0, int16x8_t a1, int16x4_t a2) { return vqrdmlshq_lane_s16(a0, a1, a2, 1); }
__attribute__((target("v8.1a"))) auto f_vqrdmlshq_lane_s32(int32x4_t a0, int32x4_t a1, int32x2_t a2) { return vqrdmlshq_lane_s32(a0, a1, a2, 1); }
auto f_vqshlu_n_s16(int16x4_t a0) { return vqshlu_n_s16(a0, 1); }
auto f_vqshluq_n_s16(int16x8_t a0) { return vqshluq_n_s16(a0, 1); }
__attribute__((target("v8.5a,neon"))) auto f_vrnd32x_f32(float32x2_t a0) { return vrnd32x_f32(a0); }
__attribute__((target("v8.5a,neon"))) auto f_vrnd32xq_f32(float32x4_t a0) { return vrnd32xq_f32(a0); }
__attribute__((target("v8.5a,neon"))) auto f_vrnd32z_f32(float32x2_t a0) { return vrnd32z_f32(a0); }
__attribute__((target("v8.5a,neon"))) auto f_vrnd32zq_f32(float32x4_t a0) { return vrnd32zq_f32(a0); }
__attribute__((target("v8.5a,neon"))) auto f_vrnd64x_f32(float32x2_t a0) { return vrnd64x_f32(a0); }
__attribute__((target("v8.5a,neon"))) auto f_vrnd64xq_f32(float32x4_t a0) { return vrnd64xq_f32(a0); }
__attribute__((target("v8.5a,neon"))) auto f_vrnd64z_f32(float32x2_t a0) { return vrnd64z_f32(a0); }
__attribute__((target("v8.5a,neon"))) auto f_vrnd64zq_f32(float32x4_t a0) { return vrnd64zq_f32(a0); }
__attribute__((target("fullfp16"))) auto f_vrndah_f16(float16_t a0) { return vrndah_f16(a0); }
__attribute__((target("fullfp16,neon"))) auto f_vrndi_f16(float16x4_t a0) { return vrndi_f16(a0); }
__attribute__((target("fullfp16"))) auto f_vrndih_f16(float16_t a0) { return vrndih_f16(a0); }
__attribute__((target("neon"))) auto f_vrndiq_f32(float32x4_t a0) { return vrndiq_f32(a0); }
__attribute__((target("fullfp16"))) auto f_vrndnh_f16(float16_t a0) { return vrndnh_f16(a0); }
__attribute__((target("fullfp16"))) auto f_vrndph_f16(float16_t a0) { return vrndph_f16(a0); }
__attribute__((target("neon"))) auto f_vrndx_f32(float32x2_t a0) { return vrndx_f32(a0); }
__attribute__((target("fullfp16,neon"))) auto f_vrndxq_f16(float16x8_t a0) { return vrndxq_f16(a0); }
auto f_vfmad_lane_f64(float64_t a0, float64_t a1, float64x1_t a2) { return vfmad_lane_f64(a0, a1, a2, 0); }
auto f_vfmad_laneq_f64(float64_t a0, float64_t a1, float64x2_t a2) { return vfmad_laneq_f64(a0, a1, a2, 1); }
__attribute__((target("fullfp16,neon"))) auto f_vfmah_laneq_f16(float16_t a0, float16_t a1, float16x8_t a2) { return vfmah_laneq_f16(a0, a1, a2, 1); }
auto f_vfmas_lane_f32(float32_t a0, float32_t a1, float32x2_t a2) { return vfmas_lane_f32(a0, a1, a2, 1); }
auto f_vmul_lane_f64(float64x1_t a0, float64x1_t a1) { return vmul_lane_f64(a0, a1, 0); }
__attribute__((target("fullfp16,neon"))) auto f_vmulxh_lane_f16(float16_t a0, float16x4_t a1) { return vmulxh_lane_f16(a0, a1, 1); }
__attribute__((target("fullfp16,neon"))) auto f_vmulxh_laneq_f16(float16_t a0, float16x8_t a1) { return vmulxh_laneq_f16(a0, a1, 1); }
auto f_vqdmlalh_laneq_s16(int32_t a0, int16_t a1, int16x8_t a2) { return vqdmlalh_laneq_s16(a0, a1, a2, 1); }
auto f_vqdmlals_laneq_s32(int64_t a0, int32_t a1, int32x4_t a2) { return vqdmlals_laneq_s32(a0, a1, a2, 1); }
auto f_vqdmlslh_lane_s16(int32_t a0, int16_t a1, int16x4_t a2) { return vqdmlslh_lane_s16(a0, a1, a2, 1); }
auto f_vqdmlsls_lane_s32(int64_t a0, int32_t a1, int32x2_t a2) { return vqdmlsls_lane_s32(a0, a1, a2, 1); }
auto f_vqdmlsls_laneq_s32(int64_t a0, int32_t a1, int32x4_t a2) { return vqdmlsls_laneq_s32(a0, a1, a2, 1); }
auto f_vqdmulh_laneq_s16(int16x4_t a0, int16x8_t a1) { return vqdmulh_laneq_s16(a0, a1, 1); }
auto f_vqdmulhq_laneq_s16(int16x8_t a0, int16x8_t a1) { return vqdmulhq_laneq_s16(a0, a1, 1); }
auto f_vqrdmulh_lane_s16(int16x4_t a0, int16x4_t a1) { return vqrdmulh_lane_s16(a0, a1, 1); }
auto f_vqrdmulhq_lane_s16(int16x8_t a0, int16x4_t a1) { return vqrdmulhq_lane_s16(a0, a1, 1); }
__attribute__((target("neon"))) auto f_vaddhn_s16(int16x8_t a0, int16x8_t a1) { return vaddhn_s16(a0, a1); }
__attribute__((target("neon"))) auto f_vraddhn_u32(uint32x4_t a0, uint32x4_t a1) { return vraddhn_u32(a0, a1); }
__attribute__((target("neon"))) auto f_vcls_s8(int8x8_t a0) { return vcls_s8(a0); }
__attribute__((target("neon"))) auto f_vsubhn_high_u64(uint32x2_t a0, uint64x2_t a1, uint64x2_t a2) { return vsubhn_high_u64(a0, a1, a2); }
