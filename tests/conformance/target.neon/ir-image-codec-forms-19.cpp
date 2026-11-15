// [driver] Six NEON forms found building open-source image codecs, verbatim:
// the by-lane saturating doubling multiplies `vq[r]dmulh[q]_lane_s16`
// (`sq[r]dmulh.lane`), the rounding shift-right-accumulate `vrsraq_n_u16`
// (`urshl` by `splat(-n)`, then `add`), the lane store `vst4_lane_u8`
// (`st4lane`) and the replicating load `vld3_dup_u8` (`ld3r`), as clang 19
// lowers them (spec/codegen.md "AArch64 NEON builtins"). WP-1421.
//@ requires: clang<22
//@ requires: target-aarch64
//@ expect: ir
//@ args: -ftrivial-auto-var-init=uninitialized
#include <arm_neon.h>
int16x8_t a(int16x8_t x, int16x4_t c) { return vqrdmulhq_lane_s16(x, c, 1); }
int16x8_t b(int16x8_t x, int16x4_t c) { return vqdmulhq_lane_s16(x, c, 2); }
int16x4_t d(int16x4_t x, int16x4_t c) { return vqdmulh_lane_s16(x, c, 3); }
uint16x8_t e(uint16x8_t acc, uint16x8_t x) { return vrsraq_n_u16(acc, x, 4); }
void f(uint8_t *p, uint8x8x4_t v) { vst4_lane_u8(p, v, 0); }
uint8x8x3_t g(const uint8_t *p) { return vld3_dup_u8(p); }
