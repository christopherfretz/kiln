// [driver] The NEON structure loads and stores de-interleave and
// interleave exactly as their scalar equivalents: `vldN[q]` reads N*n
// contiguous elements into N vectors (element i of vector j is element
// i*N+j), `vstN[q]` writes them back interleaved, and `vsli[q]_n` shifts
// each lane left by n and inserts it over the destination's low n bits
// (spec/landed/sema/WP-799.md "NEON structure loads and stores"). WP-799.
//@ expect: run
//@ requires: target-aarch64
//@ strict-diags
//@ stdout: ok 10
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
  uint16_t h[17];
  for (int i = 0; i < 17; ++i) h[i] = uint16_t(i * 11 + 1);
  uint16x8x2_t a = vld2q_u16(h + 1);
  bool ok = true;
  for (int i = 0; i < 8; ++i) ok &= a.val[0][i] == h[1 + 2 * i] && a.val[1][i] == h[2 + 2 * i];
  check(ok, "vld2q_u16");

  uint8_t b[24];
  for (int i = 0; i < 24; ++i) b[i] = uint8_t(i * 7 + 3);
  uint8x8x3_t t = vld3_u8(b);
  ok = true;
  for (int i = 0; i < 8; ++i)
    for (int j = 0; j < 3; ++j) ok &= t.val[j][i] == b[3 * i + j];
  check(ok, "vld3_u8");

  float f[16];
  for (int i = 0; i < 16; ++i) f[i] = float(i) * 0.5f;
  float32x4x4_t q = vld4q_f32(f);
  ok = true;
  for (int i = 0; i < 4; ++i)
    for (int j = 0; j < 4; ++j) ok &= q.val[j][i] == f[4 * i + j];
  check(ok, "vld4q_f32");

  uint16_t h2[16] = {};
  vst2q_u16(h2, a);
  ok = true;
  for (int i = 0; i < 16; ++i) ok &= h2[i] == h[1 + i];
  check(ok, "vst2q_u16");

  uint8_t b2[24] = {};
  vst3_u8(b2, t);
  ok = true;
  for (int i = 0; i < 24; ++i) ok &= b2[i] == b[i];
  check(ok, "vst3_u8");

  float f2[16] = {};
  vst4q_f32(f2, q);
  ok = true;
  for (int i = 0; i < 16; ++i) ok &= f2[i] == f[i];
  check(ok, "vst4q_f32");

  int32_t w[8] = {1, -2, 3, -4, 5, -6, 7, -8};
  int32x2x4_t s4 = vld4_s32(w);
  int32_t w2[8] = {};
  vst4_s32(w2, s4);
  ok = s4.val[1][1] == -6 && s4.val[3][0] == -4;
  for (int i = 0; i < 8; ++i) ok &= w2[i] == w[i];
  check(ok, "vld4_s32/vst4_s32");

  uint8x8_t x = vdup_n_u8(0xA5), y = vdup_n_u8(0x0F);
  uint8x8_t r = vsli_n_u8(x, y, 4);
  check(r[0] == 0xF5 && r[7] == 0xF5, "vsli_n_u8");

  uint32x4_t xx = vdupq_n_u32(0x12345678u), yy = vdupq_n_u32(0x1u);
  uint32x4_t rr = vsliq_n_u32(xx, yy, 31);
  check(rr[0] == 0x92345678u && rr[3] == 0x92345678u, "vsliq_n_u32");

  int64x1_t z = vsli_n_s64(vdup_n_s64(5), vdup_n_s64(9), 0);
  check(z[0] == 9, "vsli_n_s64 0");

  if (fails == 0) printf("ok %d\n", checks);
}
