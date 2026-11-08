// [driver] `vld2[q]`/`vst2[q]` lower to `ld2`/`st2` (the loaded pair stored through the result
// pointer), `vshll_n` to an extend and `shl`, `vrshrn_n` to `rshrn`, `vqmovun` to `sqxtun`,
// `vpadal[q]` to `{s,u}addlp` plus `add`, `vqneg`/`vqabs` to `sqneg`/`sqabs`, as clang 19's
// `EmitAArch64BuiltinExpr` (OpenCV's carotene HAL; WP-803).
//@ requires: clang<22
//@ requires: target-aarch64
//@ expect: ir
//@ args: -ftrivial-auto-var-init=uninitialized
#include <arm_neon.h>
auto f_vld2q_u8(const unsigned char *p) { return vld2q_u8(p); }
auto f_vld2_f32(const float *p) { return vld2_f32(p); }
void f_vst2q_s16(short *p, int16x8x2_t v) { vst2q_s16(p, v); }
void f_vst2_u8(unsigned char *p, uint8x8x2_t v) { vst2_u8(p, v); }
auto f_vshll_n_s8(int8x8_t a) { return vshll_n_s8(a, 3); }
auto f_vshll_n_u16(uint16x4_t a) { return vshll_n_u16(a, 16); }
auto f_vrshrn_n_s16(int16x8_t a) { return vrshrn_n_s16(a, 2); }
auto f_vrshrn_n_u32(uint32x4_t a) { return vrshrn_n_u32(a, 5); }
auto f_vqmovun_s16(int16x8_t a) { return vqmovun_s16(a); }
auto f_vqmovun_s64(int64x2_t a) { return vqmovun_s64(a); }
auto f_vpadal_s8(int16x4_t a, int8x8_t b) { return vpadal_s8(a, b); }
auto f_vpadalq_u16(uint32x4_t a, uint16x8_t b) { return vpadalq_u16(a, b); }
auto f_vqneg_s8(int8x8_t a) { return vqneg_s8(a); }
auto f_vqnegq_s32(int32x4_t a) { return vqnegq_s32(a); }
auto f_vqabsq_s16(int16x8_t a) { return vqabsq_s16(a); }
