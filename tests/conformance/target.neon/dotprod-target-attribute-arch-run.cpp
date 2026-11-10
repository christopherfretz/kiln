// [driver] `vdotq_u32` called from a function with
// `target("arch=armv8.2-a+dotprod")` and no `-march=`: the attribute's
// features make the intrinsic available there (and its always_inline
// callee's features a subset), and it lowers to `udot`: each lane is
// r[i] + the dot product of four byte lanes (spec/driver.md "Target
// features", target attribute). WP-1187.
//@ expect: run
//@ requires: target-aarch64
//@ strict-diags
//@ stdout: 30 174 446 846
extern "C" int printf(const char *, ...);
#include <arm_neon.h>

__attribute__((target("arch=armv8.2-a+dotprod"))) uint32x4_t dot(uint32x4_t r, uint8x16_t a, uint8x16_t b) {
  return vdotq_u32(r, a, b);
}

int main() {
  uint8_t x[16];
  for (int i = 0; i < 16; ++i) x[i] = (uint8_t)(i + 1);
  uint32x4_t r = dot(vdupq_n_u32(0), vld1q_u8(x), vld1q_u8(x));
  printf("%u %u %u %u\n", vgetq_lane_u32(r, 0), vgetq_lane_u32(r, 1), vgetq_lane_u32(r, 2), vgetq_lane_u32(r, 3));
}
