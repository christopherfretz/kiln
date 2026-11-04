// [driver] At -O2 the NEON lane and permute builtins agree with scalar
// code: `vext` takes n lanes from the concatenation starting at the
// immediate, `vzip`/`vuzp`/`vtrn` interleave, de-interleave and transpose,
// `vqtbl1q` indexes a 16-byte table (0 out of range), `vshr_n` shifts each
// lane (logical for unsigned, arithmetic for signed), `vget`/`vset`/`vdup`
// lane read, write and splat one lane, and an 8-byte control-group match
// (`vld1_u8`, `vceq_u8`, `vget_lane_u64`) yields one 0x80 byte per hit
// (spec/landed/codegen/WP-628.md "NEON load/store builtins"). WP-628.
//@ expect: run
//@ requires: target-aarch64
//@ args: -O2
//@ strict-diags
//@ stdout: ok 10 8000000000800080
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

__attribute__((noinline)) uint64_t match(const int8_t *ctrl, uint8_t h) {
  uint8x8_t g = vld1_u8(reinterpret_cast<const uint8_t *>(ctrl));
  uint8x8_t eq = vceq_u8(g, vdup_n_u8(h));
  return vget_lane_u64(vreinterpret_u64_u8(eq), 0) & 0x8080808080808080ull;
}

int main() {
  uint8_t a8[16], b8[16];
  for (int i = 0; i < 16; ++i) {
    a8[i] = uint8_t(i);
    b8[i] = uint8_t(0x80 | i);
  }
  uint8x16_t a = vld1q_u8(a8), b = vld1q_u8(b8);

  uint8x16_t e = vextq_u8(a, b, 13);
  bool ok = true;
  for (int i = 0; i < 16; ++i) ok &= e[i] == (13 + i < 16 ? a8[13 + i] : b8[13 + i - 16]);
  check(ok, "vextq_u8");

  int16x8_t x = {0, 1, 2, 3, 4, 5, 6, 7}, y = {10, 11, 12, 13, 14, 15, 16, 17};
  int16x8x2_t z = vzipq_s16(x, y), u = vuzpq_s16(x, y), t = vtrnq_s16(x, y);
  ok = true;
  for (int i = 0; i < 8; ++i) {
    int k = i / 2;
    ok &= z.val[0][i] == (i % 2 ? y[k] : x[k]) && z.val[1][i] == (i % 2 ? y[4 + k] : x[4 + k]);
  }
  check(ok, "vzipq_s16");
  ok = true;
  for (int i = 0; i < 8; ++i) {
    int j0 = 2 * i, j1 = 2 * i + 1;
    ok &= u.val[0][i] == (j0 < 8 ? x[j0] : y[j0 - 8]) && u.val[1][i] == (j1 < 8 ? x[j1] : y[j1 - 8]);
  }
  check(ok, "vuzpq_s16");
  ok = true;
  for (int i = 0; i < 8; i += 2) {
    ok &= t.val[0][i] == x[i] && t.val[0][i + 1] == y[i];
    ok &= t.val[1][i] == x[i + 1] && t.val[1][i + 1] == y[i + 1];
  }
  check(ok, "vtrnq_s16");

  uint8x16_t idx = {15, 0, 3, 200, 16, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 255};
  uint8x16_t r = vqtbl1q_u8(b, idx);
  ok = true;
  for (int i = 0; i < 16; ++i) ok &= r[i] == (idx[i] < 16 ? b8[idx[i]] : 0);
  check(ok, "vqtbl1q_u8");

  uint8x8_t s = vshr_n_u8(vget_high_u8(b), 3);
  int32x4_t w = {-64, 64, -1, 7};
  int32x4_t neg = vshrq_n_s32(w, 2);
  ok = neg[0] == -16 && neg[1] == 16 && neg[2] == -1 && neg[3] == 1;
  for (int i = 0; i < 8; ++i) ok &= s[i] == (b8[8 + i] >> 3);
  check(ok, "vshr_n");
  uint8x8_t all = vshr_n_u8(vget_low_u8(b), 8);
  int8x8_t sign = vshr_n_s8(vreinterpret_s8_u8(vget_low_u8(b)), 8);
  ok = vget_lane_u64(vreinterpret_u64_u8(all), 0) == 0;
  for (int i = 0; i < 8; ++i) ok &= sign[i] == -1;
  check(ok, "vshr_n by the width");

  float32x4_t f = {1, 2, 3, 4};
  f = vsetq_lane_f32(9.5f, f, 2);
  check(vgetq_lane_f32(f, 2) == 9.5f && vgetq_lane_f32(f, 3) == 4.0f, "vsetq_lane_f32");
  float32x4_t sp = vdupq_laneq_f32(f, 2);
  check(sp[0] == 9.5f && sp[3] == 9.5f && vdups_laneq_f32(f, 1) == 2.0f, "vdupq_laneq_f32");

  int8_t ctrl[8] = {5, -128, 5, 7, -2, 9, 1, 5};
  uint64_t m = match(ctrl, 5);
  uint64_t want = 0;
  for (int i = 0; i < 8; ++i)
    if (ctrl[i] == 5) want |= 0x80ull << (8 * i);
  check(m == want, "control-group match");

  if (fails == 0) printf("ok %d %016llx\n", checks, (unsigned long long)m);
}
