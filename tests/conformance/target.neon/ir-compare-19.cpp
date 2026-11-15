// [driver] The comparisons: against zero (`icmp`/`fcmp` with zero as the type
// the operand had before the header's bitcast, `sext` to the result), the
// absolute compares `facge`/`facgt` (`vcale`/`vcalt` with the operands
// swapped; the `half` forms an `i32` call truncated), and the scalar compares
// and `vtstd` as clang 19 lowers them (spec/codegen.md "AArch64 NEON
// builtins"; from a census of arm_neon.h, scripts/neon-intrinsic-census.py).
// WP-1421.
//@ requires: clang<22
//@ requires: target-aarch64
//@ expect: ir
//@ args: -ftrivial-auto-var-init=uninitialized
#include <arm_fp16.h>
#include <arm_neon.h>
__attribute__((target("fullfp16,neon"))) auto f_vceqz_f16(float16x4_t a0) { return vceqz_f16(a0); }
__attribute__((target("neon"))) auto f_vceqzd_f64(float64_t a0) { return vceqzd_f64(a0); }
__attribute__((target("fullfp16"))) auto f_vceqzh_f16(float16_t a0) { return vceqzh_f16(a0); }
__attribute__((target("neon"))) auto f_vceqzq_f32(float32x4_t a0) { return vceqzq_f32(a0); }
__attribute__((target("neon"))) auto f_vcgez_f32(float32x2_t a0) { return vcgez_f32(a0); }
__attribute__((target("neon"))) auto f_vcgezd_s64(int64_t a0) { return vcgezd_s64(a0); }
__attribute__((target("neon"))) auto f_vcgezq_f32(float32x4_t a0) { return vcgezq_f32(a0); }
__attribute__((target("fullfp16,neon"))) auto f_vcgtz_f16(float16x4_t a0) { return vcgtz_f16(a0); }
__attribute__((target("neon"))) auto f_vcgtzd_s64(int64_t a0) { return vcgtzd_s64(a0); }
__attribute__((target("neon"))) auto f_vcgtzq_f32(float32x4_t a0) { return vcgtzq_f32(a0); }
__attribute__((target("fullfp16,neon"))) auto f_vclez_f16(float16x4_t a0) { return vclez_f16(a0); }
__attribute__((target("neon"))) auto f_vclezd_s64(int64_t a0) { return vclezd_s64(a0); }
__attribute__((target("fullfp16,neon"))) auto f_vclezq_f16(float16x8_t a0) { return vclezq_f16(a0); }
__attribute__((target("fullfp16,neon"))) auto f_vcltz_f16(float16x4_t a0) { return vcltz_f16(a0); }
__attribute__((target("neon"))) auto f_vcltzd_f64(float64_t a0) { return vcltzd_f64(a0); }
__attribute__((target("fullfp16,neon"))) auto f_vcltzq_f16(float16x8_t a0) { return vcltzq_f16(a0); }
__attribute__((target("fullfp16,neon"))) auto f_vcage_f16(float16x4_t a0, float16x4_t a1) { return vcage_f16(a0, a1); }
__attribute__((target("neon"))) auto f_vcage_f32(float32x2_t a0, float32x2_t a1) { return vcage_f32(a0, a1); }
__attribute__((target("fullfp16"))) auto f_vcageh_f16(float16_t a0, float16_t a1) { return vcageh_f16(a0, a1); }
__attribute__((target("neon"))) auto f_vcageq_f32(float32x4_t a0, float32x4_t a1) { return vcageq_f32(a0, a1); }
__attribute__((target("fullfp16,neon"))) auto f_vcagt_f16(float16x4_t a0, float16x4_t a1) { return vcagt_f16(a0, a1); }
__attribute__((target("neon"))) auto f_vcagt_f32(float32x2_t a0, float32x2_t a1) { return vcagt_f32(a0, a1); }
__attribute__((target("fullfp16"))) auto f_vcagth_f16(float16_t a0, float16_t a1) { return vcagth_f16(a0, a1); }
__attribute__((target("neon"))) auto f_vcagtq_f32(float32x4_t a0, float32x4_t a1) { return vcagtq_f32(a0, a1); }
__attribute__((target("fullfp16,neon"))) auto f_vcale_f16(float16x4_t a0, float16x4_t a1) { return vcale_f16(a0, a1); }
__attribute__((target("neon"))) auto f_vcale_f32(float32x2_t a0, float32x2_t a1) { return vcale_f32(a0, a1); }
__attribute__((target("fullfp16"))) auto f_vcaleh_f16(float16_t a0, float16_t a1) { return vcaleh_f16(a0, a1); }
__attribute__((target("neon"))) auto f_vcaleq_f32(float32x4_t a0, float32x4_t a1) { return vcaleq_f32(a0, a1); }
__attribute__((target("fullfp16,neon"))) auto f_vcalt_f16(float16x4_t a0, float16x4_t a1) { return vcalt_f16(a0, a1); }
__attribute__((target("neon"))) auto f_vcalt_f32(float32x2_t a0, float32x2_t a1) { return vcalt_f32(a0, a1); }
__attribute__((target("fullfp16"))) auto f_vcalth_f16(float16_t a0, float16_t a1) { return vcalth_f16(a0, a1); }
__attribute__((target("neon"))) auto f_vcaltq_f32(float32x4_t a0, float32x4_t a1) { return vcaltq_f32(a0, a1); }
__attribute__((target("neon"))) auto f_vceqd_f64(float64_t a0, float64_t a1) { return vceqd_f64(a0, a1); }
__attribute__((target("neon"))) auto f_vceqd_s64(int64_t a0, int64_t a1) { return vceqd_s64(a0, a1); }
__attribute__((target("fullfp16"))) auto f_vceqh_f16(float16_t a0, float16_t a1) { return vceqh_f16(a0, a1); }
__attribute__((target("neon"))) auto f_vcged_f64(float64_t a0, float64_t a1) { return vcged_f64(a0, a1); }
__attribute__((target("neon"))) auto f_vcged_s64(int64_t a0, int64_t a1) { return vcged_s64(a0, a1); }
__attribute__((target("fullfp16"))) auto f_vcgeh_f16(float16_t a0, float16_t a1) { return vcgeh_f16(a0, a1); }
__attribute__((target("neon"))) auto f_vcgtd_f64(float64_t a0, float64_t a1) { return vcgtd_f64(a0, a1); }
__attribute__((target("neon"))) auto f_vcgtd_s64(int64_t a0, int64_t a1) { return vcgtd_s64(a0, a1); }
__attribute__((target("fullfp16"))) auto f_vcgth_f16(float16_t a0, float16_t a1) { return vcgth_f16(a0, a1); }
__attribute__((target("neon"))) auto f_vcled_f64(float64_t a0, float64_t a1) { return vcled_f64(a0, a1); }
__attribute__((target("neon"))) auto f_vcled_s64(int64_t a0, int64_t a1) { return vcled_s64(a0, a1); }
__attribute__((target("fullfp16"))) auto f_vcleh_f16(float16_t a0, float16_t a1) { return vcleh_f16(a0, a1); }
__attribute__((target("neon"))) auto f_vcltd_f64(float64_t a0, float64_t a1) { return vcltd_f64(a0, a1); }
__attribute__((target("neon"))) auto f_vcltd_s64(int64_t a0, int64_t a1) { return vcltd_s64(a0, a1); }
__attribute__((target("fullfp16"))) auto f_vclth_f16(float16_t a0, float16_t a1) { return vclth_f16(a0, a1); }
__attribute__((target("neon"))) auto f_vtstd_s64(int64_t a0, int64_t a1) { return vtstd_s64(a0, a1); }
