// [driver] `vbfdotq_f32` from a function with `target("bf16")` under
// `-march=armv8.6-a+bf16` lowers to `llvm.aarch64.neon.bfdot` (and the
// operands' `vcvtq_low/high_bf16_f32` to `bfcvtn`/`bfcvtn2`): each lane
// of the result is r[i] + a[2i]*b[2i] + a[2i+1]*b[2i+1] (spec/codegen.md
// "Target features"). WP-558.
//@ expect: run
//@ requires: target-aarch64
//@ args: -march=armv8.6-a+bf16
//@ strict-diags
//@ stdout: 18 55 20 57
extern "C" int printf(const char *, ...);
#include <arm_neon.h>

__attribute__((target("bf16"))) float32x4_t dot(float32x4_t r, float32x4_t lo, float32x4_t hi) {
  bfloat16x8_t a = vcvtq_high_bf16_f32(vcvtq_low_bf16_f32(lo), hi);
  bfloat16x8_t b = vcvtq_high_bf16_f32(vcvtq_low_bf16_f32(hi), lo);
  return vbfdotq_f32(r, a, b);
}

int main() {
  float32x4_t r = {1, 2, 3, 4};
  float32x4_t lo = {1, 2, 3, 4};
  float32x4_t hi = {5, 6, 7, 8};
  float32x4_t d = dot(r, lo, hi);
  printf("%g %g %g %g\n", d[0], d[1], d[2], d[3]);
}
