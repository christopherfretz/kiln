// [driver] The rest of the NEON arithmetic builtins agree with scalar code
// at -O0: the immediate shifts (`vshl_n`, `vshrn_n`, `vsra_n` at full
// width, `vsri_n`), `vmovl`/`vqmovn` widen and saturate-narrow, `vfma`
// by lane (`vfma_laneq_f64` too), the float width `vcvt`s, the pairwise
// `vpmin`/`vpmax`, `vabd`, `vqadd`, `vminnm`/`vmaxnm` (a NaN loses),
// the reciprocal estimates and steps, the `vrnd*` rounding modes, `vcnt`,
// `vclz` and the complex `vcmla` rotations; `_f16` twins included
// (spec/landed/codegen/WP-628.md "NEON arithmetic builtins"). WP-628.
//@ expect: run
//@ requires: target-aarch64
//@ args: -O0 -march=armv8.3-a+fp16
//@ strict-diags
//@ stdout: ok 26
extern "C" int printf(const char *, ...);
#include <arm_neon.h>
#include <stdint.h>

static int fails;
static int checks;
static void check(bool c, const char *what) {
  ++checks;
  if (!c) {
    ++fails;
    printf("FAIL %s\n", what);
  }
}

int main() {
  uint8_t a8[16], b8[16], r8[16];
  for (int i = 0; i < 16; ++i) {
    a8[i] = uint8_t(i * 37 + 5);
    b8[i] = uint8_t(250 - i * 29);
  }
  uint8x16_t a = vld1q_u8(a8), b = vld1q_u8(b8);
  bool ok = true;

  vst1q_u8(r8, vshlq_n_u8(a, 3));
  for (int i = 0; i < 16; ++i)
    ok &= r8[i] == uint8_t(a8[i] << 3);
  vst1q_u8(r8, vsraq_n_u8(a, b, 8));
  for (int i = 0; i < 16; ++i)
    ok &= r8[i] == a8[i];
  vst1q_u8(r8, vsraq_n_u8(a, b, 2));
  for (int i = 0; i < 16; ++i)
    ok &= r8[i] == uint8_t(a8[i] + (b8[i] >> 2));
  check(ok, "vshlq_n/vsraq_n_u8");

  int8_t s8[8];
  vst1_s8(s8, vsra_n_s8(vdup_n_s8(1), vcreate_s8(0x7f80ff0001020304ull), 8));
  check(s8[0] == 1 && s8[4] == 1 && s8[5] == 0 && s8[6] == 0 && s8[7] == 1, "vsra_n_s8 by 8");

  vst1q_u8(r8, vsriq_n_u8(a, b, 3));
  ok = true;
  for (int i = 0; i < 16; ++i)
    ok &= r8[i] == uint8_t((a8[i] & 0xe0) | (b8[i] >> 3));
  check(ok, "vsriq_n_u8");

  uint16x8_t w = vmovl_u8(vget_low_u8(a));
  uint16_t w16[8];
  vst1q_u16(w16, w);
  ok = true;
  for (int i = 0; i < 8; ++i)
    ok &= w16[i] == a8[i];
  vst1_u8(r8, vshrn_n_u16(vshlq_n_u16(w, 4), 2));
  for (int i = 0; i < 8; ++i)
    ok &= r8[i] == uint8_t((a8[i] << 4) >> 2);
  check(ok, "vmovl_u8/vshrn_n_u16");

  int16_t m16[8];
  vst1q_s16(m16, vmovl_s8(vcreate_s8(0x80ff7f0102030405ull)));
  check(m16[7] == -128 && m16[6] == -1 && m16[5] == 127, "vmovl_s8");

  vst1_s8(s8, vqmovn_s16(vcombine_s16(vcreate_s16(0x7fff8000007f0080ull), vdup_n_s16(-5))));
  check(s8[0] == 127 && s8[1] == 127 && s8[2] == -128 && s8[3] == 127 && s8[4] == -5,
        "vqmovn_s16");
  uint16_t q16[4];
  vst1_u16(q16, vqmovn_u32(vcombine_u32(vcreate_u32(0x000100000000ffffull), vdup_n_u32(7))));
  check(q16[0] == 0xffff && q16[1] == 0xffff && q16[2] == 7, "vqmovn_u32");

  float32x4_t fa = {1, 2, 3, 4}, fb = {0.5f, -1, 2, 8};
  float32x2_t fc = {10, 20};
  float f[4];
  vst1q_f32(f, vfmaq_lane_f32(fa, fb, fc, 1));
  check(f[0] == 11 && f[1] == -18 && f[2] == 43 && f[3] == 164, "vfmaq_lane_f32");
  vst1q_f32(f, vfmaq_laneq_f32(fa, fb, fb, 3));
  check(f[0] == 5 && f[1] == -6 && f[2] == 19 && f[3] == 68, "vfmaq_laneq_f32");
  float64x2_t dc = {3, 5};
  check(vget_lane_f64(vfma_laneq_f64(vdup_n_f64(1), vdup_n_f64(2), dc, 1), 0) == 11,
        "vfma_laneq_f64");
  float16x8_t ha = vdupq_n_f16(1), hb = vdupq_n_f16(2);
  float16x4_t hc = {1, 2, 3, 4};
  check(vgetq_lane_f16(vfmaq_lane_f16(ha, hb, hc, 2), 5) == float16_t(7), "vfmaq_lane_f16");

  float32x4_t g = vcvt_f32_f16(vcvt_f16_f32(fb));
  float64x2_t d = vcvt_f64_f32(vget_low_f32(fa));
  vst1q_f32(f, g);
  check(f[0] == 0.5f && f[3] == 8 && vgetq_lane_f64(d, 1) == 2 &&
            vget_lane_f32(vcvt_f32_f64(dc), 1) == 5,
        "vcvt f16/f32/f64");

  vst1_u8(r8, vpmin_u8(vget_low_u8(a), vget_high_u8(a)));
  ok = true;
  for (int i = 0; i < 8; ++i) {
    uint8_t x = a8[2 * i], y = a8[2 * i + 1];
    ok &= r8[i] == (x < y ? x : y);
  }
  vst1_u8(r8, vpmax_u8(vget_low_u8(a), vget_high_u8(a)));
  for (int i = 0; i < 8; ++i) {
    uint8_t x = a8[2 * i], y = a8[2 * i + 1];
    ok &= r8[i] == (x > y ? x : y);
  }
  check(ok, "vpmin/vpmax_u8");

  vst1q_u8(r8, vabdq_u8(a, b));
  ok = true;
  for (int i = 0; i < 16; ++i)
    ok &= r8[i] == (a8[i] > b8[i] ? a8[i] - b8[i] : b8[i] - a8[i]);
  vst1q_u8(r8, vqaddq_u8(a, b));
  for (int i = 0; i < 16; ++i)
    ok &= r8[i] == (a8[i] + b8[i] > 255 ? 255 : a8[i] + b8[i]);
  check(ok, "vabdq/vqaddq_u8");
  check(vget_lane_s16(vqadd_s16(vdup_n_s16(30000), vdup_n_s16(30000)), 0) == 32767 &&
            vgetq_lane_f32(vabdq_f32(fa, fb), 1) == 3,
        "vqadd_s16/vabdq_f32");

  float32x2_t nan = vdup_n_f32(__builtin_nanf(""));
  float32x2_t one = vdup_n_f32(1);
  check(vget_lane_f32(vminnm_f32(nan, one), 0) == 1 && vget_lane_f32(vmaxnm_f32(one, nan), 1) == 1,
        "vminnm/vmaxnm_f32");

  float32x4_t e = vrecpeq_f32(fb);
  vst1q_f32(f, e);
  check(f[0] > 1.99f && f[0] < 2.01f && f[3] > 0.124f && f[3] < 0.126f, "vrecpeq_f32");
  vst1q_f32(f, vrecpsq_f32(fa, fb));
  check(f[0] == 1.5f && f[1] == 4 && f[3] == -30, "vrecpsq_f32");
  vst1q_f32(f, vrsqrteq_f32(vdupq_n_f32(4)));
  check(f[0] > 0.49f && f[0] < 0.51f, "vrsqrteq_f32");
  check(vget_lane_f32(vrsqrts_f32(vdup_n_f32(2), vdup_n_f32(3)), 0) == -1.5f, "vrsqrts_f32");
  check(vget_lane_u32(vrecpe_u32(vdup_n_u32(0x7fffffffu)), 0) == 0xffffffffu, "vrecpe_u32");

  float32x4_t r = {-2.5f, -1.5f, 0.5f, 2.7f};
  float t[4], u[4], n[4], mm[4], aa[4];
  vst1q_f32(t, vrndq_f32(r));
  vst1q_f32(u, vrndpq_f32(r));
  vst1q_f32(n, vrndnq_f32(r));
  vst1q_f32(mm, vrndmq_f32(r));
  vst1q_f32(aa, vrndaq_f32(r));
  check(t[0] == -2 && t[3] == 2 && u[0] == -2 && u[3] == 3 && n[0] == -2 && n[1] == -2 &&
            n[2] == 0 && mm[0] == -3 && mm[2] == 0 && aa[0] == -3 && aa[2] == 1,
        "vrnd*q_f32");
  check(vget_lane_f16(vrnda_f16(vdup_n_f16(1.5)), 0) == float16_t(2), "vrnda_f16");

  vst1q_u8(r8, vcntq_u8(a));
  ok = true;
  for (int i = 0; i < 16; ++i)
    ok &= r8[i] == __builtin_popcount(a8[i]);
  vst1q_u8(r8, vclzq_u8(a));
  for (int i = 0; i < 16; ++i)
    ok &= r8[i] == (a8[i] ? __builtin_clz(a8[i]) - 24 : 8);
  check(ok, "vcntq/vclzq_u8");
  check(vgetq_lane_u32(vclzq_u32(vdupq_n_u32(0)), 2) == 32 &&
            vget_lane_s16(vclz_s16(vdup_n_s16(-1)), 0) == 0,
        "vclz u32/s16");

  // (1 + 2i) * (3 + 4i) = -5 + 10i, accumulated onto 1 + 1i.
  float32x2_t acc = {1, 1}, x = {1, 2}, y = {3, 4};
  float32x2_t z = vcmla_rot90_f32(vcmla_f32(acc, x, y), x, y);
  check(vget_lane_f32(z, 0) == -4 && vget_lane_f32(z, 1) == 11, "vcmla/vcmla_rot90_f32");

  if (fails == 0)
    printf("ok %d\n", checks);
  return fails;
}
