// [driver] The NEON arithmetic builtins agree with scalar code at -O0:
// `vpaddq` adds adjacent pairs of the concatenation, `vqsubq` saturates
// (unsigned at 0, signed at the type's limits), `vmaxq`/`vminq`/`vabsq`/
// `vsqrtq` work lane by lane, `vfmaq` is `a + b * c`, `vbslq` takes each
// bit from the second operand where the mask is set, `vtst[q]` is all ones
// where the operands share a set bit, `vcvtq` converts
// (to integer toward zero), `vmaxvq`/`vminvq`/`vaddvq` reduce across
// lanes, and `vmull_p64` is the carry-less product; `_f16` twins included
// (spec/landed/codegen/WP-628.md "NEON arithmetic builtins"). WP-628.
//@ expect: run
//@ requires: target-aarch64
//@ args: -O0 -march=armv8.2-a+fp16+aes
//@ strict-diags
//@ stdout: ok 22
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

static unsigned __int128 clmul(uint64_t a, uint64_t b) {
  unsigned __int128 r = 0;
  for (int i = 0; i < 64; ++i)
    if (b >> i & 1)
      r ^= static_cast<unsigned __int128>(a) << i;
  return r;
}

int main() {
  uint8_t a8[16], b8[16], r8[16];
  for (int i = 0; i < 16; ++i) {
    a8[i] = uint8_t(i * 17);
    b8[i] = uint8_t(255 - i * 9);
  }
  uint8x16_t a = vld1q_u8(a8), b = vld1q_u8(b8);

  vst1q_u8(r8, vpaddq_u8(a, b));
  bool ok = true;
  for (int i = 0; i < 16; ++i) {
    const uint8_t *s = i < 8 ? a8 : b8;
    int j = (i % 8) * 2;
    ok &= r8[i] == uint8_t(s[j] + s[j + 1]);
  }
  check(ok, "vpaddq_u8");

  vst1q_u8(r8, vqsubq_u8(a, b));
  ok = true;
  for (int i = 0; i < 16; ++i)
    ok &= r8[i] == (a8[i] > b8[i] ? a8[i] - b8[i] : 0);
  check(ok, "vqsubq_u8");

  int16_t s16a[8] = {-32768, 32767, 100, -100, 0, 1, -1, 20000};
  int16_t s16b[8] = {1, -1, 200, -200, 0, -32768, 32767, -20000};
  int16_t s16r[8];
  vst1q_s16(s16r, vqsubq_s16(vld1q_s16(s16a), vld1q_s16(s16b)));
  ok = true;
  for (int i = 0; i < 8; ++i) {
    int d = s16a[i] - s16b[i];
    ok &= s16r[i] == (d > 32767 ? 32767 : d < -32768 ? -32768 : d);
  }
  check(ok, "vqsubq_s16");

  uint8_t mx = 0, mn = 255;
  for (int i = 0; i < 16; ++i) {
    mx = a8[i] > mx ? a8[i] : mx;
    mn = a8[i] < mn ? a8[i] : mn;
  }
  check(vmaxvq_u8(a) == mx && vminvq_u8(a) == mn, "vmaxvq/vminvq_u8");

  float fa[4] = {1.5f, -2.5f, 4.0f, -0.25f};
  float fb[4] = {2.0f, 3.0f, -1.0f, 8.0f};
  float fc[4] = {0.5f, 0.25f, 2.0f, -4.0f};
  float fr[4];
  float32x4_t va = vld1q_f32(fa), vb = vld1q_f32(fb), vc = vld1q_f32(fc);
  check(vaddvq_f32(va) == 2.75f, "vaddvq_f32");
  check(vaddvq_u64(vdupq_n_u64(0x8000000000000001ull)) == 2, "vaddvq_u64");

  vst1q_f32(fr, vfmaq_f32(va, vb, vc));
  ok = true;
  for (int i = 0; i < 4; ++i)
    ok &= fr[i] == fa[i] + fb[i] * fc[i];
  check(ok, "vfmaq_f32");

  vst1q_f32(fr, vabsq_f32(va));
  check(fr[0] == 1.5f && fr[1] == 2.5f && fr[3] == 0.25f, "vabsq_f32");

  vst1q_f32(fr, vmaxq_f32(va, vb));
  check(fr[0] == 2.0f && fr[1] == 3.0f && fr[2] == 4.0f && fr[3] == 8.0f, "vmaxq_f32");
  vst1q_f32(fr, vminq_f32(va, vb));
  check(fr[0] == 1.5f && fr[1] == -2.5f && fr[2] == -1.0f && fr[3] == -0.25f, "vminq_f32");

  float sq[4] = {4.0f, 9.0f, 0.25f, 2.0f};
  vst1q_f32(fr, vsqrtq_f32(vld1q_f32(sq)));
  check(fr[0] == 2.0f && fr[1] == 3.0f && fr[2] == 0.5f && fr[3] * fr[3] > 1.99f, "vsqrtq_f32");

  uint32_t m[4] = {0xffffffffu, 0, 0xffff0000u, 0x80000000u};
  uint32x4_t vm = vld1q_u32(m);
  vst1q_f32(fr, vbslq_f32(vm, va, vb));
  uint32_t ua[4], ub[4], ur[4];
  vst1q_u32(ua, vreinterpretq_u32_f32(va));
  vst1q_u32(ub, vreinterpretq_u32_f32(vb));
  vst1q_u32(ur, vreinterpretq_u32_f32(vld1q_f32(fr)));
  ok = true;
  for (int i = 0; i < 4; ++i)
    ok &= ur[i] == ((m[i] & ua[i]) | (~m[i] & ub[i]));
  check(ok, "vbslq_f32");

  int32_t si[4] = {-7, 0, 123456, -2147483647};
  vst1q_f32(fr, vcvtq_f32_s32(vld1q_s32(si)));
  ok = true;
  for (int i = 0; i < 4; ++i)
    ok &= fr[i] == float(si[i]);
  check(ok, "vcvtq_f32_s32");

  int32_t ir[4];
  vst1q_s32(ir, vcvtq_s32_f32(va));
  check(ir[0] == 1 && ir[1] == -2 && ir[2] == 4 && ir[3] == 0, "vcvtq_s32_f32");
  uint32_t uo[4];
  vst1q_u32(uo, vcvtq_u32_f32(vb));
  check(uo[0] == 2 && uo[1] == 3 && uo[2] == 0 && uo[3] == 8, "vcvtq_u32_f32");

  int32_t ab[4] = {-5, 5, 0, -2147483647};
  vst1q_s32(ir, vabsq_s32(vld1q_s32(ab)));
  check(ir[0] == 5 && ir[1] == 5 && ir[2] == 0 && ir[3] == 2147483647, "vabsq_s32");

  uint16_t u16[4];
  vst1_u16(u16, vmax_u16(vcreate_u16(0x0001fffe00800003ull), vcreate_u16(0x00020001ffff0000ull)));
  check(u16[0] == 3 && u16[1] == 0xffff && u16[2] == 0xfffe && u16[3] == 2, "vmax_u16");

  vst1q_u8(r8, vtstq_u8(a, b));
  ok = true;
  for (int i = 0; i < 16; ++i)
    ok &= r8[i] == ((a8[i] & b8[i]) ? 0xff : 0);
  check(ok, "vtstq_u8");
  int32_t t32[2];
  vst1_s32(t32, vreinterpret_s32_u32(vtst_s32(vcreate_s32(0x0000000180000000ull),
                                              vcreate_s32(0x00000002ffffffffull))));
  check(t32[0] == -1 && t32[1] == 0, "vtst_s32");

  uint64_t x = 0x87654321fedcba98ull, y = 0x0123456789abcdefull;
  unsigned __int128 p = vmull_p64(x, y);
  check(p == clmul(x, y), "vmull_p64");

  float16_t h[4] = {1.5, -3.0, 0.5, 2.0};
  float16x4_t vh = vld1_f16(h);
  check(vmaxv_f16(vh) == float16_t(2.0) && vminv_f16(vh) == float16_t(-3.0), "vmaxv/vminv_f16");
  int16_t hi[4];
  vst1_s16(hi, vcvt_s16_f16(vabs_f16(vh)));
  check(hi[0] == 1 && hi[1] == 3 && hi[2] == 0 && hi[3] == 2, "vcvt_s16_f16/vabs_f16");

  if (fails == 0)
    printf("ok %d\n", checks);
  return fails;
}
