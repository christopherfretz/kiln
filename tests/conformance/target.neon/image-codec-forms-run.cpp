// [driver] The six NEON forms found building open-source image codecs
// (`ir-image-codec-forms-19.cpp`) compute what a scalar reference computes:
// `vqdmulh` is `sat((2 * x * c) >> 16)`, `vqrdmulh` adds `1 << 15` first,
// `vrsraq_n_u16` adds `(x + (1 << (n - 1))) >> n`, `vst4_lane_u8` stores
// lane 0 of each of the four vectors, and `vld3_dup_u8` replicates three
// bytes (spec/codegen.md "AArch64 NEON builtins"). WP-1421.
//@ expect: run
//@ requires: target-aarch64
//@ strict-diags
//@ stdout: ok 6
extern "C" int printf(const char *, ...);
#include <arm_neon.h>

static int16_t sat16(long long v) {
  return v > 32767 ? 32767 : v < -32768 ? -32768 : static_cast<int16_t>(v);
}
static int16_t qdmulh(int16_t x, int16_t c) { return sat16((2LL * x * c) >> 16); }
static int16_t qrdmulh(int16_t x, int16_t c) {
  return sat16((2LL * x * c + (1LL << 15)) >> 16);
}

int main() {
  int16_t xs[8] = {1000, -2000, 32767, -32768, 12345, -1, 7, 30000};
  int16_t cs[4] = {-32768, 16384, -12345, 321};
  int16x8_t x = vld1q_s16(xs);
  int16x4_t c = vld1_s16(cs);
  int ok = 0;
  int16_t r[8];
  vst1q_s16(r, vqrdmulhq_lane_s16(x, c, 1));
  bool good = true;
  for (int i = 0; i < 8; ++i)
    good &= r[i] == qrdmulh(xs[i], cs[1]);
  ok += good;
  vst1q_s16(r, vqdmulhq_lane_s16(x, c, 2));
  good = true;
  for (int i = 0; i < 8; ++i)
    good &= r[i] == qdmulh(xs[i], cs[2]);
  ok += good;
  vst1_s16(r, vqdmulh_lane_s16(vget_low_s16(x), c, 3));
  good = true;
  for (int i = 0; i < 4; ++i)
    good &= r[i] == qdmulh(xs[i], cs[3]);
  // `vqdmulh` of -32768 by -32768 saturates.
  good &= vgetq_lane_s16(vqdmulhq_lane_s16(x, c, 0), 3) == 32767;
  ok += good;
  uint16_t as[8] = {0, 1, 2, 65535, 100, 200, 300, 400};
  uint16_t bs[8] = {7, 8, 9, 15, 65535, 24, 40, 1};
  uint16_t u[8];
  vst1q_u16(u, vrsraq_n_u16(vld1q_u16(as), vld1q_u16(bs), 4));
  good = true;
  for (int i = 0; i < 8; ++i)
    good &= u[i] == static_cast<uint16_t>(as[i] + ((bs[i] + 8u) >> 4));
  ok += good;
  uint8_t src[32];
  for (int i = 0; i < 32; ++i)
    src[i] = static_cast<uint8_t>(3 * i + 1);
  uint8x8x4_t v = {{vld1_u8(src), vld1_u8(src + 8), vld1_u8(src + 16), vld1_u8(src + 24)}};
  uint8_t out[5] = {0, 0, 0, 0, 0xee};
  vst4_lane_u8(out, v, 0);
  ok += out[0] == src[0] && out[1] == src[8] && out[2] == src[16] && out[3] == src[24] &&
        out[4] == 0xee;
  uint8x8x3_t t = vld3_dup_u8(src + 5);
  good = true;
  for (int i = 0; i < 8; ++i) {
    uint8_t l[3] = {0, 0, 0};
    uint8_t m[8];
    vst1_u8(m, t.val[0]);
    l[0] = m[i];
    vst1_u8(m, t.val[1]);
    l[1] = m[i];
    vst1_u8(m, t.val[2]);
    l[2] = m[i];
    good &= l[0] == src[5] && l[1] == src[6] && l[2] == src[7];
  }
  ok += good;
  printf("ok %d\n", ok);
}
