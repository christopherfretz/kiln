// [driver] More NEON builtins OpenCV's carotene HAL uses agree with scalar code at -O0: the
// de-interleaving `vld3`/`vld4`/`vst3`/`vst4`, the directed-rounding conversions
// `vcvt{n,a,m,p}`, the halving `vhadd`/`vrhadd`/`vhsub`, the saturating narrowing shifts
// `vq(r)shr(u)n_n`, the saturating `vqshl_n`, the rounding `vrshr_n` and the table lookups
// `vtbl1`/`vtbl2`, and the widening sums `vaddlv` (WP-803).
//@ expect: run
//@ requires: target-aarch64
//@ args: -O0
//@ strict-diags
//@ stdout: ld3 0 3 6 | 1 4 | 2 23
//@ stdout: ld4 0 4 | 1 5 | 2 6 | 3 63
//@ stdout: st3 1 10 100 2 20 200 | 4 40 400
//@ stdout: st4 0 1 2 3 4 5 6 7 | 7 15 23 31
//@ stdout: cvtn 0 2 -2 3 | cvta 1 -1 3 -3 | cvtm 0 -1 2 -3 | cvtp 1 0 3 -2 | cvtnu 2 4
//@ stdout: hadd -64 2 1 | rhadd 128 2 255 | hsub -2 100
//@ stdout: qrshrn 1 -1 125 -125 | qshrn 1000 1 | qrshrun 0 255 13 | qshrun 0 65535
//@ stdout: qshl 127 -128 8 | qshlu 65535 12 | rshr 1 -1 3 | rshru 2 128
//@ stdout: tbl1 3 0 7 0 | tbl2 15 0 9 2
//@ stdout: addlv 4080 -16 131070 -6000000000
#include <arm_neon.h>
#include <cstdio>

int main() {
  unsigned char b[64];
  for (int i = 0; i < 64; ++i) b[i] = (unsigned char)i;
  uint8x8x3_t l3 = vld3_u8(b);
  std::printf("ld3 %d %d %d | %d %d | %d %d\n", vget_lane_u8(l3.val[0], 0),
              vget_lane_u8(l3.val[0], 1), vget_lane_u8(l3.val[0], 2), vget_lane_u8(l3.val[1], 0),
              vget_lane_u8(l3.val[1], 1), vget_lane_u8(l3.val[2], 0), vget_lane_u8(l3.val[2], 7));
  uint8x16x4_t l4 = vld4q_u8(b);
  std::printf("ld4 %d %d | %d %d | %d %d | %d %d\n", vgetq_lane_u8(l4.val[0], 0),
              vgetq_lane_u8(l4.val[0], 1), vgetq_lane_u8(l4.val[1], 0),
              vgetq_lane_u8(l4.val[1], 1), vgetq_lane_u8(l4.val[2], 0),
              vgetq_lane_u8(l4.val[2], 1), vgetq_lane_u8(l4.val[3], 0),
              vgetq_lane_u8(l4.val[3], 15));

  short s3[12];
  int16x4x3_t v3;
  v3.val[0] = (int16x4_t){1, 2, 3, 4};
  v3.val[1] = (int16x4_t){10, 20, 30, 40};
  v3.val[2] = (int16x4_t){100, 200, 300, 400};
  vst3_s16(s3, v3);
  std::printf("st3 %d %d %d %d %d %d | %d %d %d\n", s3[0], s3[1], s3[2], s3[3], s3[4], s3[5],
              s3[9], s3[10], s3[11]);
  unsigned int u4[32];
  uint32x4x4_t v4;
  v4.val[0] = (uint32x4_t){0, 4, 0, 7};
  v4.val[1] = (uint32x4_t){1, 5, 0, 15};
  v4.val[2] = (uint32x4_t){2, 6, 0, 23};
  v4.val[3] = (uint32x4_t){3, 7, 0, 31};
  vst4q_u32(u4, v4);
  std::printf("st4 %u %u %u %u %u %u %u %u | %u %u %u %u\n", u4[0], u4[1], u4[2], u4[3], u4[4],
              u4[5], u4[6], u4[7], u4[12], u4[13], u4[14], u4[15]);

  float32x4_t f = {0.5f, 1.5f, -2.5f, 2.7f};
  int32x4_t n = vcvtnq_s32_f32(f);
  float32x2_t g = {0.5f, -0.5f};
  int32x2_t a = vcvta_s32_f32(g);
  float32x4_t h = {2.5f, -2.5f, 0.2f, -2.2f};
  int32x4_t ha = vcvtaq_s32_f32(h);
  float32x4_t m4 = {0.7f, -0.2f, 2.9f, -2.1f};
  int32x4_t m = vcvtmq_s32_f32(m4);
  int32x4_t p = vcvtpq_s32_f32(m4);
  float32x2_t ug = {2.5f, 3.5f};
  uint32x2_t un = vcvtn_u32_f32(ug);
  std::printf("cvtn %d %d %d %d | cvta %d %d %d %d | cvtm %d %d %d %d | cvtp %d %d %d %d | "
              "cvtnu %u %u\n",
              vgetq_lane_s32(n, 0), vgetq_lane_s32(n, 1), vgetq_lane_s32(n, 2),
              vgetq_lane_s32(n, 3), vget_lane_s32(a, 0), vget_lane_s32(a, 1),
              vgetq_lane_s32(ha, 0), vgetq_lane_s32(ha, 1), vgetq_lane_s32(m, 0),
              vgetq_lane_s32(m, 1), vgetq_lane_s32(m, 2), vgetq_lane_s32(m, 3),
              vgetq_lane_s32(p, 0), vgetq_lane_s32(p, 1), vgetq_lane_s32(p, 2),
              vgetq_lane_s32(p, 3), vget_lane_u32(un, 0), vget_lane_u32(un, 1));

  int8x8_t x8 = {-128, 3, 1, 0, 0, 0, 0, 0}, y8 = {0, 1, 2, 0, 0, 0, 0, 0};
  int8x8_t hd = vhadd_s8(x8, y8);
  uint8x16_t ux = vdupq_n_u8(255), uy = vdupq_n_u8(1);
  ux = vsetq_lane_u8(2, ux, 1);
  uy = vsetq_lane_u8(1, uy, 1);
  ux = vsetq_lane_u8(255, ux, 2);
  uy = vsetq_lane_u8(255, uy, 2);
  uint8x16_t rh = vrhaddq_u8(ux, uy);
  int16x4_t hx = {-3, 200, 0, 0}, hy = {0, 0, 0, 0};
  int16x4_t hs = vhsub_s16(hx, hy);
  std::printf("hadd %d %d %d | rhadd %d %d %d | hsub %d %d\n", vget_lane_s8(hd, 0),
              vget_lane_s8(hd, 1), vget_lane_s8(hd, 2), vgetq_lane_u8(rh, 0),
              vgetq_lane_u8(rh, 1), vgetq_lane_u8(rh, 2), vget_lane_s16(hs, 0),
              vget_lane_s16(hs, 1));

  int16x8_t q16 = {6, -6, 1000, -1000, 0, 0, 0, 0};
  int8x8_t qr = vqrshrn_n_s16(q16, 3);
  uint32x4_t q32 = {4000, 6, 0, 0};
  uint16x4_t qs = vqshrn_n_u32(q32, 2);
  int16x8_t qu16 = {-50, 5000, 100, 0, 0, 0, 0, 0};
  uint8x8_t qru = vqrshrun_n_s16(qu16, 3);
  int32x4_t qu32 = {-7, 1 << 30, 0, 0};
  uint16x4_t qsu = vqshrun_n_s32(qu32, 4);
  std::printf("qrshrn %d %d %d %d | qshrn %d %d | qrshrun %d %d %d | qshrun %d %d\n",
              vget_lane_s8(qr, 0), vget_lane_s8(qr, 1), vget_lane_s8(qr, 2),
              vget_lane_s8(qr, 3), vget_lane_u16(qs, 0), vget_lane_u16(qs, 1),
              vget_lane_u8(qru, 0), vget_lane_u8(qru, 1), vget_lane_u8(qru, 2),
              vget_lane_u16(qsu, 0), vget_lane_u16(qsu, 1));

  int8x8_t shi = {100, -100, 1, 0, 0, 0, 0, 0};
  int8x8_t sh = vqshl_n_s8(shi, 3);
  uint16x8_t shui = {40000, 3, 0, 0, 0, 0, 0, 0};
  uint16x8_t shu = vqshlq_n_u16(shui, 2);
  int16x4_t rsi = {2, -3, 10, 0};
  int16x4_t rs = vrshr_n_s16(rsi, 2);
  uint8x16_t rsui = vsetq_lane_u8(255, vdupq_n_u8(3), 1);
  uint8x16_t rsu = vrshrq_n_u8(rsui, 1);
  std::printf("qshl %d %d %d | qshlu %d %d | rshr %d %d %d | rshru %d %d\n",
              vget_lane_s8(sh, 0), vget_lane_s8(sh, 1), vget_lane_s8(sh, 2),
              vgetq_lane_u16(shu, 0), vgetq_lane_u16(shu, 1), vget_lane_s16(rs, 0),
              vget_lane_s16(rs, 1), vget_lane_s16(rs, 2), vgetq_lane_u8(rsu, 0),
              vgetq_lane_u8(rsu, 1));

  uint8x8_t t0 = {0, 1, 2, 3, 4, 5, 6, 7}, t1 = {8, 9, 10, 11, 12, 13, 14, 15};
  uint8x8_t i1 = {3, 9, 7, 200, 0, 0, 0, 0};
  uint8x8_t r1 = vtbl1_u8(t0, i1);
  uint8x8x2_t tt;
  tt.val[0] = t0;
  tt.val[1] = t1;
  uint8x8_t i2 = {15, 16, 9, 2, 0, 0, 0, 0};
  uint8x8_t r2 = vtbl2_u8(tt, i2);
  std::printf("tbl1 %d %d %d %d | tbl2 %d %d %d %d\n", vget_lane_u8(r1, 0), vget_lane_u8(r1, 1),
              vget_lane_u8(r1, 2), vget_lane_u8(r1, 3), vget_lane_u8(r2, 0),
              vget_lane_u8(r2, 1), vget_lane_u8(r2, 2), vget_lane_u8(r2, 3));

  uint8x16_t au = vdupq_n_u8(255);
  int8x8_t as = vdup_n_s8(-2);
  uint16x4_t au16 = {65535, 65535, 0, 0};
  int32x4_t as32 = vdupq_n_s32(-1500000000);
  std::printf("addlv %d %d %u %lld\n", vaddlvq_u8(au), vaddlv_s8(as), vaddlv_u16(au16),
              (long long)vaddlvq_s32(as32));
}
