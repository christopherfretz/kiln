// [driver] The widening multiplies `vmull_{s,u,p}*` and `vmull_high_*`
// agree with scalar code at -O0: each lane's product at twice the width,
// signed, unsigned, or carry-less for `poly8`
// (spec/landed/sema/WP-724.md "NEON widening multiply"). WP-724.
//@ expect: run
//@ requires: target-aarch64
//@ args: -O0
//@ strict-diags
//@ stdout: ok 5
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

static uint16_t clmul(uint8_t a, uint8_t b) {
  uint16_t r = 0;
  for (int i = 0; i < 8; ++i)
    if (b & (1 << i))
      r ^= (uint16_t)(a << i);
  return r;
}

int main() {
  uint32x2_t ua = {0xffffffffu, 3}, ub = {0xfffffffeu, 5};
  uint64x2_t u = vmull_u32(ua, ub);
  check(vgetq_lane_u64(u, 0) == 0xffffffffull * 0xfffffffeull && vgetq_lane_u64(u, 1) == 15, "u32");
  int16x4_t sa = {-32768, 7, -3, 100}, sb = {-32768, -7, 3, 300};
  int32x4_t s = vmull_s16(sa, sb);
  check(vgetq_lane_s32(s, 0) == 1073741824 && vgetq_lane_s32(s, 1) == -49 &&
            vgetq_lane_s32(s, 2) == -9 && vgetq_lane_s32(s, 3) == 30000,
        "s16");
  int8x8_t ca = {-128, 1, 2, 3, 4, 5, 6, 7}, cb = {-128, -1, 2, 3, 4, 5, 6, -7};
  int16x8_t c = vmull_s8(ca, cb);
  check(vgetq_lane_s16(c, 0) == 16384 && vgetq_lane_s16(c, 1) == -1 && vgetq_lane_s16(c, 7) == -49, "s8");
  poly8x8_t pa = {0x53, 0xff, 1, 2, 3, 4, 5, 6}, pb = {0xca, 0xff, 1, 2, 3, 4, 5, 6};
  poly16x8_t p = vmull_p8(pa, pb);
  check(vgetq_lane_p16(p, 0) == clmul(0x53, 0xca) && vgetq_lane_p16(p, 1) == clmul(0xff, 0xff) &&
            vgetq_lane_p16(p, 7) == clmul(6, 6),
        "p8");
  uint32x4_t ha = {1, 2, 0x80000000u, 9}, hb = {1, 2, 4, 9};
  uint64x2_t h = vmull_high_u32(ha, hb);
  check(vgetq_lane_u64(h, 0) == 0x200000000ull && vgetq_lane_u64(h, 1) == 81, "high_u32");
  printf("ok %d\n", checks - fails);
  return fails;
}
