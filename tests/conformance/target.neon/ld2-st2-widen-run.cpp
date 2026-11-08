// [driver] The NEON builtins OpenCV's carotene HAL uses beyond WP-628/WP-792 agree with scalar
// code at -O0: the de-interleaving `vld2`/`vst2` (`ld2`/`st2`), the widening shift `vshll_n`,
// the rounding narrowing shift `vrshrn_n`, the unsigned-saturating narrow `vqmovun`, the
// pairwise add-accumulate `vpadal` and the saturating `vqneg`/`vqabs` (WP-803).
//@ expect: run
//@ requires: target-aarch64
//@ args: -O0
//@ strict-diags
//@ stdout: ld2 0 2 4 6 8 10 12 14 16 18 20 22 24 26 28 30 | 1 31
//@ stdout: st2 0 100 1 101 2 102 3 103 | 7 107
//@ stdout: ld2f 0.5 2.5 | 1.5 3.5
//@ stdout: shll -4 -508 508 | 1020 0 4
//@ stdout: rshrn 1 1 2 -1 0 -1 | 0 1
//@ stdout: qmovun 0 0 255 255 7 100 0 255
//@ stdout: padal 101 -3 8 10 | 510 3
//@ stdout: qneg 127 -127 0 5 | qabs 127 127 0 5
#include <arm_neon.h>
#include <cstdio>

int main() {
  unsigned char b[32];
  for (int i = 0; i < 32; ++i) b[i] = (unsigned char)i;
  uint8x16x2_t d = vld2q_u8(b);
  std::printf("ld2");
  for (int i = 0; i < 16; ++i) std::printf(" %d", d.val[0][i]);
  std::printf(" | %d %d\n", d.val[1][0], d.val[1][15]);

  short out[16];
  int16x8_t a = {0, 1, 2, 3, 4, 5, 6, 7};
  int16x8_t c = vaddq_s16(a, vdupq_n_s16(100));
  int16x8x2_t ac = {{a, c}};
  vst2q_s16(out, ac);
  std::printf("st2 %d %d %d %d %d %d %d %d | %d %d\n", out[0], out[1], out[2], out[3], out[4],
              out[5], out[6], out[7], out[14], out[15]);

  float fb[4] = {0.5f, 1.5f, 2.5f, 3.5f};
  float32x2x2_t fd = vld2_f32(fb);
  std::printf("ld2f %g %g | %g %g\n", fd.val[0][0], fd.val[0][1], fd.val[1][0], fd.val[1][1]);

  int8x8_t s8 = {-1, -127, 127, 0, 0, 0, 0, 1};
  int16x8_t w = vshll_n_s8(s8, 2);
  uint8x8_t u8 = {255, 0, 1, 0, 0, 0, 0, 0};
  uint16x8_t uw = vshll_n_u8(u8, 2);
  std::printf("shll %d %d %d | %d %d %d\n", w[0], w[1], w[2], uw[0], uw[1], uw[2]);

  int16x8_t r = {1, 2, 3, -2, -1, -3, 0, 1};
  int8x8_t rn = vrshrn_n_s16(r, 1);
  uint16x4_t ur = {0, 1, 2, 3};
  uint8x8_t urn = vrshrn_n_u16(vcombine_u16(ur, ur), 2);
  std::printf("rshrn %d %d %d %d %d %d | %d %d\n", rn[0], rn[1], rn[2], rn[3], rn[4], rn[5], urn[0],
              urn[2]);

  int16x8_t q = {-1, -300, 255, 256, 7, 100, 0, 32767};
  uint8x8_t qu = vqmovun_s16(q);
  std::printf("qmovun %d %d %d %d %d %d %d %d\n", qu[0], qu[1], qu[2], qu[3], qu[4], qu[5], qu[6],
              qu[7]);

  int16x4_t acc = {100, -5, 0, 10};
  int8x8_t pa = {0, 1, -1, 3, 4, 4, 0, 0};
  int16x4_t pr = vpadal_s8(acc, pa);
  uint16x8_t uacc = vdupq_n_u16(0);
  uint8x16_t upa = vdupq_n_u8(255);
  uint16x8_t upr = vpadalq_u8(uacc, upa);
  upr = vpadalq_u8(upr, vdupq_n_u8(0));
  uint16x4_t lo = vget_low_u16(vpadalq_u8(vdupq_n_u16(1), vdupq_n_u8(1)));
  std::printf("padal %d %d %d %d | %d %d\n", pr[0], pr[1], pr[2], pr[3], upr[0], lo[0]);

  int8x8_t ng = {-128, 127, 0, -5, 0, 0, 0, 0};
  int8x8_t n1 = vqneg_s8(ng);
  int8x8_t a1 = vqabs_s8(ng);
  std::printf("qneg %d %d %d %d | qabs %d %d %d %d\n", n1[0], n1[1], n1[2], n1[3], a1[0], a1[1],
              a1[2], a1[3]);
}
