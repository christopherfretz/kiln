// [driver] `__builtin_neon_vmull_v`, reached by `vmull_{s,u,p}*` and
// `vmull_high_*`, emits clang 19's IR: one `llvm.aarch64.neon.{s,u,p}mull`
// of the wide vector type the type code names (signed, unsigned or poly
// element), on both operands bitcast to the half-width lanes
// (spec/landed/sema/WP-724.md "NEON widening multiply"). WP-724.
//@ requires: clang<22
//@ requires: target-aarch64
//@ expect: ir
//@ args: -ftrivial-auto-var-init=uninitialized
#include <arm_neon.h>
auto f_vmull_s8(int8x8_t a, int8x8_t b) { return vmull_s8(a, b); }
auto f_vmull_s16(int16x4_t a, int16x4_t b) { return vmull_s16(a, b); }
auto f_vmull_s32(int32x2_t a, int32x2_t b) { return vmull_s32(a, b); }
auto f_vmull_u8(uint8x8_t a, uint8x8_t b) { return vmull_u8(a, b); }
auto f_vmull_u16(uint16x4_t a, uint16x4_t b) { return vmull_u16(a, b); }
auto f_vmull_u32(uint32x2_t a, uint32x2_t b) { return vmull_u32(a, b); }
auto f_vmull_p8(poly8x8_t a, poly8x8_t b) { return vmull_p8(a, b); }
auto f_vmull_high_s16(int16x8_t a, int16x8_t b) { return vmull_high_s16(a, b); }
auto f_vmull_high_u32(uint32x4_t a, uint32x4_t b) { return vmull_high_u32(a, b); }
