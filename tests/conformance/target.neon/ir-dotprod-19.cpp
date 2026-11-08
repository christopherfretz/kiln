// [driver] `__builtin_neon_vdot[q]_{s,u}32`, reached by `vdot[q]_{s,u}32`
// and their `_lane[q]` forms, emits clang 19's IR: one
// `llvm.aarch64.neon.{s,u}dot` overloaded on the accumulator and the byte
// vector, its result returned as is (spec/landed/codegen/WP-628.md). WP-803.
//@ requires: clang<22
//@ requires: target-aarch64
//@ expect: ir
//@ args: -march=armv8.2-a+dotprod -ftrivial-auto-var-init=uninitialized
#include <arm_neon.h>
auto f_vdot_s32(int32x2_t r, int8x8_t a, int8x8_t b) { return vdot_s32(r, a, b); }
auto f_vdot_u32(uint32x2_t r, uint8x8_t a, uint8x8_t b) { return vdot_u32(r, a, b); }
auto f_vdotq_s32(int32x4_t r, int8x16_t a, int8x16_t b) { return vdotq_s32(r, a, b); }
auto f_vdotq_u32(uint32x4_t r, uint8x16_t a, uint8x16_t b) { return vdotq_u32(r, a, b); }
auto f_vdot_lane_u32(uint32x2_t r, uint8x8_t a, uint8x8_t b) { return vdot_lane_u32(r, a, b, 1); }
auto f_vdotq_laneq_s32(int32x4_t r, int8x16_t a, int8x16_t b) { return vdotq_laneq_s32(r, a, b, 3); }
