// [driver] The conversions: with a rounding mode (`fcvt{a,m,n,p,z}{s,u}`, a
// 16-bit result an `i32` call truncated), scalar `sitofp`/`uitofp`, the
// fixed-point `_n` forms (`vcvtfxs2fp`, `vcvtfp2fxs`, ...; a 16-bit integer
// source extended to `i32`), and `vcvtx` as clang 19 lowers them
// (spec/codegen.md "AArch64 NEON builtins"; from a census of arm_neon.h,
// scripts/neon-intrinsic-census.py). WP-1421.
//@ requires: clang<22
//@ requires: target-aarch64
//@ expect: ir
//@ args: -ftrivial-auto-var-init=uninitialized
#include <arm_fp16.h>
#include <arm_neon.h>
__attribute__((target("fullfp16,neon"))) auto f_vcvta_s16_f16(float16x4_t a0) { return vcvta_s16_f16(a0); }
__attribute__((target("fullfp16"))) auto f_vcvtah_s16_f16(float16_t a0) { return vcvtah_s16_f16(a0); }
__attribute__((target("fullfp16"))) auto f_vcvtah_u64_f16(float16_t a0) { return vcvtah_u64_f16(a0); }
__attribute__((target("neon"))) auto f_vcvtd_f64_s64(int64_t a0) { return vcvtd_f64_s64(a0); }
__attribute__((target("fullfp16"))) auto f_vcvth_f16_s32(int32_t a0) { return vcvth_f16_s32(a0); }
__attribute__((target("fullfp16"))) auto f_vcvth_s16_f16(float16_t a0) { return vcvth_s16_f16(a0); }
__attribute__((target("fullfp16"))) auto f_vcvth_u64_f16(float16_t a0) { return vcvth_u64_f16(a0); }
__attribute__((target("fullfp16"))) auto f_vcvtmh_s16_f16(float16_t a0) { return vcvtmh_s16_f16(a0); }
__attribute__((target("fullfp16"))) auto f_vcvtmh_u64_f16(float16_t a0) { return vcvtmh_u64_f16(a0); }
__attribute__((target("neon"))) auto f_vcvtms_u32_f32(float32_t a0) { return vcvtms_u32_f32(a0); }
__attribute__((target("fullfp16"))) auto f_vcvtnh_s16_f16(float16_t a0) { return vcvtnh_s16_f16(a0); }
__attribute__((target("fullfp16"))) auto f_vcvtnh_u64_f16(float16_t a0) { return vcvtnh_u64_f16(a0); }
__attribute__((target("fullfp16,neon"))) auto f_vcvtp_s16_f16(float16x4_t a0) { return vcvtp_s16_f16(a0); }
__attribute__((target("fullfp16"))) auto f_vcvtph_s32_f16(float16_t a0) { return vcvtph_s32_f16(a0); }
__attribute__((target("fullfp16,neon"))) auto f_vcvtpq_s16_f16(float16x8_t a0) { return vcvtpq_s16_f16(a0); }
__attribute__((target("neon"))) auto f_vcvts_f32_u32(uint32_t a0) { return vcvts_f32_u32(a0); }
__attribute__((target("fullfp16,neon"))) auto f_vcvt_n_f16_s16(int16x4_t a0) { return vcvt_n_f16_s16(a0, 1); }
auto f_vcvt_n_f32_s32(int32x2_t a0) { return vcvt_n_f32_s32(a0, 1); }
auto f_vcvt_n_s32_f32(float32x2_t a0) { return vcvt_n_s32_f32(a0, 1); }
__attribute__((target("fullfp16,neon"))) auto f_vcvt_n_u16_f16(float16x4_t a0) { return vcvt_n_u16_f16(a0, 1); }
auto f_vcvtd_n_f64_s64(int64_t a0) { return vcvtd_n_f64_s64(a0, 1); }
auto f_vcvtd_n_s64_f64(float64_t a0) { return vcvtd_n_s64_f64(a0, 1); }
__attribute__((target("fullfp16"))) auto f_vcvth_n_f16_s32(int32_t a0) { return vcvth_n_f16_s32(a0, 1); }
__attribute__((target("fullfp16"))) auto f_vcvth_n_f16_u16(uint16_t a0) { return vcvth_n_f16_u16(a0, 1); }
__attribute__((target("fullfp16"))) auto f_vcvth_n_s16_f16(float16_t a0) { return vcvth_n_s16_f16(a0, 1); }
__attribute__((target("fullfp16"))) auto f_vcvth_n_s64_f16(float16_t a0) { return vcvth_n_s64_f16(a0, 1); }
__attribute__((target("fullfp16"))) auto f_vcvth_n_u64_f16(float16_t a0) { return vcvth_n_u64_f16(a0, 1); }
__attribute__((target("fullfp16,neon"))) auto f_vcvtq_n_f16_u16(uint16x8_t a0) { return vcvtq_n_f16_u16(a0, 1); }
__attribute__((target("fullfp16,neon"))) auto f_vcvtq_n_s16_f16(float16x8_t a0) { return vcvtq_n_s16_f16(a0, 1); }
auto f_vcvtq_n_s64_f64(float64x2_t a0) { return vcvtq_n_s64_f64(a0, 1); }
auto f_vcvtq_n_u64_f64(float64x2_t a0) { return vcvtq_n_u64_f64(a0, 1); }
auto f_vcvts_n_f32_u32(uint32_t a0) { return vcvts_n_f32_u32(a0, 1); }
__attribute__((target("neon"))) auto f_vcvtx_f32_f64(float64x2_t a0) { return vcvtx_f32_f64(a0); }
