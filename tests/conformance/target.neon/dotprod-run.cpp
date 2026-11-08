// [driver] The NEON dot products `vdot[q]_{s,u}32` and their lane forms
// under `-march=armv8.2-a+dotprod` lower to `llvm.aarch64.neon.{s,u}dot`:
// each 32-bit lane of the result is r[i] plus the sum of the four byte
// products of that lane (spec/landed/codegen/WP-628.md). WP-803 (OpenCV's
// NEON_DOTPROD dispatch check).
//@ expect: run
//@ requires: target-aarch64
//@ args: -march=armv8.2-a+dotprod
//@ strict-diags
//@ stdout: 1030 -6 | 31 | 2 1001 | 2 2
extern "C" int printf(const char *, ...);
#include <arm_neon.h>

int main() {
  uint8x16_t a = vdupq_n_u8(255);
  uint8x16_t b = vdupq_n_u8(1);
  uint32x4_t r = vdupq_n_u32(10);
  uint32x4_t d = vdotq_u32(r, a, b);
  int8x8_t sa = {-1, -2, -3, -4, 1, 1, 1, 1};
  int8x8_t sb = {1, 1, 1, 1, 2, 2, 2, 2};
  int32x2_t s = vdot_s32(vdup_n_s32(4), sa, sb);
  printf("%u %d | ", d[0], s[0]);
  uint8x8_t ua = {1, 2, 3, 4, 5, 6, 7, 8};
  uint32x2_t u = vdot_u32(vdup_n_u32(1), ua, ua);
  printf("%u | ", u[0]);
  int8x16_t qa = vdupq_n_s8(1);
  int8x16_t qb = {0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0};
  qb[12] = 1;
  int32x4_t qr = {1, 1001, 0, 0};
  int32x4_t l = vdotq_laneq_s32(qr, qa, qb, 3);
  printf("%d %d | ", l[0], l[1] - 1);
  uint32x2_t ul = vdot_lane_u32(vdup_n_u32(0), vdup_n_u8(1), ua, 0);
  printf("%u %u\n", ul[0] / 5, ul[1] / 5);
}
