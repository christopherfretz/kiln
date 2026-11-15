// [driver] A few builtins of each WP-1421 group compute what a scalar
// reference computes: compares against zero and absolute compares, the
// rounding and fixed-point conversions, the saturating scalar forms, the
// `d` shifts, `vaddhn`/`vraddhn`, `vqdmlal`, `vcls`, `vrbit`, `vmulx`, the
// by-lane `vqdmulh` and `vfmad_lane`, and the multi-register loads and
// stores (spec/codegen.md "AArch64 NEON builtins"). WP-1421.
//@ expect: run
//@ requires: target-aarch64
//@ strict-diags
//@ stdout: ok 12
extern "C" int printf(const char *, ...);
#include <arm_neon.h>

static long long sat(long long v, long long lo, long long hi) { return v < lo ? lo : v > hi ? hi : v; }

int main() {
  int ok = 0;
  // Compares against zero; absolute compares, `vcale` swapped.
  float fs[4] = {-1.5f, 0.0f, 2.0f, -0.0f};
  uint32_t m[4];
  vst1q_u32(m, vcltzq_f32(vld1q_f32(fs)));
  bool good = m[0] == ~0u && m[1] == 0 && m[2] == 0 && m[3] == 0;
  vst1q_u32(m, vcaleq_f32(vld1q_f32(fs), vdupq_n_f32(1.5f)));
  good &= m[0] == ~0u && m[1] == ~0u && m[2] == 0 && m[3] == ~0u;
  good &= vceqzd_s64(0) == ~0ull && vcgtzd_s64(-3) == 0 && vcled_u64(5, 3) == 0 &&
          vclts_f32(1.0f, 2.0f) == ~0u && vcagts_f32(-3.0f, 2.0f) == ~0u && vtstd_s64(6, 3) == ~0ull;
  ok += good;
  // Rounding conversions and the fixed-point forms.
  good = vcvtas_s32_f32(2.5f) == 3 && vcvtns_s32_f32(2.5f) == 2 && vcvtms_s32_f32(-2.5f) == -3 &&
         vcvtpd_s64_f64(-2.5) == -2 && vcvtd_s64_f64(-2.9) == -2 && vcvts_u32_f32(-1.0f) == 0u;
  good &= vcvts_n_s32_f32(1.25f, 4) == 20 && vcvtd_n_f64_s64(40, 3) == 5.0 &&
          vcvts_f32_u32(7u) == 7.0f;
  int32_t ci[2];
  vst1_s32(ci, vcvt_n_s32_f32(vdup_n_f32(-0.75f), 2));
  good &= ci[0] == -3 && ci[1] == -3;
  ok += good;
  // Saturating scalar forms.
  good = vqaddb_s8(100, 100) == 127 && vqsubb_u8(3, 5) == 0 && vqabsh_s16(-32768) == 32767 &&
         vqnegs_s32(-2147483647 - 1) == 2147483647 && vqmovns_s32(70000) == 32767 &&
         vqmovund_s64(-5) == 0u && vqshlb_n_u8(200, 1) == 255 && vqrshrns_n_s32(1000, 4) == 63 &&
         vqdmulhs_s32(1 << 30, 1 << 30) == (1 << 29) && vqdmullh_s16(-32768, -32768) == 2147483647;
  good &= vqdmlalh_s16(10, 300, -2) == 10 - 1200 && vqdmlsls_s32(0, 1 << 20, 1 << 20) == -(1LL << 41);
  ok += good;
  // The `d` shifts and arithmetic.
  good = vshld_n_s64(3, 4) == 48 && vshrd_n_s64(-64, 64) == -1 && vshrd_n_u64(64, 64) == 0 &&
         vsrad_n_u64(1, 8, 3) == 2 && vrsrad_n_s64(0, 7, 2) == 2 && vnegd_s64(5) == -5 &&
         vaddd_u64(~0ull, 2) == 1 && vrshrd_n_u64(5, 1) == 3 && vqrshld_s64(1, 2) == 4;
  ok += good;
  // `vaddhn`, `vraddhn`, `vrsubhn`.
  uint32_t a[4] = {0x12345678u, 0xffff8000u, 0x8000u, 0xffffffffu};
  uint32_t b[4] = {0x00010000u, 0x00008000u, 0x7fffu, 1u};
  uint16_t h[4];
  vst1_u16(h, vaddhn_u32(vld1q_u32(a), vld1q_u32(b)));
  good = true;
  for (int i = 0; i < 4; ++i)
    good &= h[i] == static_cast<uint16_t>((a[i] + b[i]) >> 16);
  vst1_u16(h, vraddhn_u32(vld1q_u32(a), vld1q_u32(b)));
  for (int i = 0; i < 4; ++i)
    good &= h[i] == static_cast<uint16_t>((a[i] + b[i] + 0x8000u) >> 16);
  ok += good;
  // `vqdmlal` and `vqdmlsl`.
  int16_t x[4] = {1000, -32768, 7, 12345};
  int16_t y[4] = {-3, -32768, 5, 2};
  int32_t acc[4] = {5, 0, -10, 2147483000};
  int32_t r[4];
  vst1q_s32(r, vqdmlal_s16(vld1q_s32(acc), vld1_s16(x), vld1_s16(y)));
  good = true;
  for (int i = 0; i < 4; ++i) {
    long long p = sat(2LL * x[i] * y[i], -2147483648LL, 2147483647LL);
    good &= r[i] == sat(acc[i] + p, -2147483648LL, 2147483647LL);
  }
  ok += good;
  // `vcls`, `vrbit`.
  int8_t cl[8] = {0, -1, 1, 64, -65, 127, -128, 3};
  int8_t cr[8];
  vst1_s8(cr, vcls_s8(vld1_s8(cl)));
  good = cr[0] == 7 && cr[1] == 7 && cr[2] == 6 && cr[3] == 0 && cr[4] == 0 && cr[5] == 0 &&
         cr[6] == 0 && cr[7] == 5;
  good &= vget_lane_u8(vrbit_u8(vdup_n_u8(0x01)), 0) == 0x80 &&
          vget_lane_u8(vrbit_u8(vdup_n_u8(0x36)), 3) == 0x6c;
  ok += good;
  // `vmulx` (0 x inf is 2), `vrndx`, `vrndi`.
  good = vmulxd_f64(0.0, __builtin_inf()) == 2.0 && vmulxs_f32(-3.0f, 2.0f) == -6.0f &&
         vgetq_lane_f32(vmulxq_f32(vdupq_n_f32(-0.0f), vdupq_n_f32(-__builtin_inff())), 1) == 2.0f;
  good &= vget_lane_f32(vrndx_f32(vdup_n_f32(2.5f)), 0) == 2.0f &&
          vget_lane_f64(vrndi_f64(vdup_n_f64(-3.5)), 0) == -4.0;
  ok += good;
  // By lane: `vqdmulh`, `vfmad_lane`, `vmul_lane_f64`, `vqdmlals_lane`.
  int16_t qa[4] = {16384, -16384, 100, 32767};
  int16x4_t qv = vld1_s16(qa);
  int16_t q[4];
  vst1_s16(q, vqdmulh_lane_s16(qv, vdup_n_s16(16384), 2));
  good = q[0] == 8192 && q[1] == -8192 && q[2] == 50 && q[3] == 16383;
  good &= vfmad_lane_f64(1.0, 2.0, vdup_n_f64(3.0), 0) == 7.0 &&
          vget_lane_f64(vmul_lane_f64(vdup_n_f64(1.5), vdup_n_f64(4.0), 0), 0) == 6.0 &&
          vqdmlals_lane_s32(1, 3, vdup_n_s32(-4), 1) == -23;
  ok += good;
  // `vld1_x3`, `vst1q_x2`, `vld2_dup`, `vld3q_lane`, `vst2_lane`.
  uint8_t src[48], dst[48];
  for (int i = 0; i < 48; ++i)
    src[i] = static_cast<uint8_t>(i * 7 + 1);
  uint8x8x3_t t3 = vld1_u8_x3(src);
  good = vget_lane_u8(t3.val[0], 1) == src[1] && vget_lane_u8(t3.val[2], 7) == src[23];
  vst1q_u8_x2(dst, vld1q_u8_x2(src + 8));
  for (int i = 0; i < 32; ++i)
    good &= dst[i] == src[8 + i];
  uint16_t s16[3] = {11, 22, 33};
  uint16x4x2_t d2 = vld2_dup_u16(s16);
  good &= vget_lane_u16(d2.val[0], 3) == 11 && vget_lane_u16(d2.val[1], 0) == 22;
  float l3[3] = {1.0f, 2.0f, 3.0f};
  float32x4x3_t f3 = {{vdupq_n_f32(0.0f), vdupq_n_f32(0.0f), vdupq_n_f32(0.0f)}};
  f3 = vld3q_lane_f32(l3, f3, 2);
  good &= vgetq_lane_f32(f3.val[0], 2) == 1.0f && vgetq_lane_f32(f3.val[2], 2) == 3.0f &&
          vgetq_lane_f32(f3.val[1], 1) == 0.0f;
  int32_t o2[2] = {0, 0};
  int32x2x2_t v2 = {{vdup_n_s32(4), vdup_n_s32(9)}};
  vst2_lane_s32(o2, v2, 1);
  good &= o2[0] == 4 && o2[1] == 9;
  ok += good;
  // `vrsra_n` and `vsqadd`/`vuqadd`.
  good = vget_lane_u8(vsqadd_u8(vdup_n_u8(250), vdup_n_s8(10)), 0) == 255 &&
         vget_lane_s8(vuqadd_s8(vdup_n_s8(-100), vdup_n_u8(250)), 5) == 127 &&
         vgetq_lane_s32(vrsraq_n_s32(vdupq_n_s32(1), vdupq_n_s32(-7), 2), 0) == -1;
  ok += good;
  // `vpaddd_f64`, `vmaxnmvq_f32`, `vpmins_f32`.
  float64_t pd[2] = {1.25, 2.5};
  float mf[4] = {1.0f, __builtin_nanf(""), 9.0f, -2.0f};
  good = vpaddd_f64(vld1q_f64(pd)) == 3.75 && vmaxnmvq_f32(vld1q_f32(mf)) == 9.0f &&
         vpmins_f32(vld1_f32(mf + 2)) == -2.0f;
  ok += good;
  printf("ok %d\n", ok);
}
