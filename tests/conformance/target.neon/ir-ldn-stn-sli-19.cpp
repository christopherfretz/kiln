// [driver] The NEON structure loads, structure stores and shift-left-and-
// insert lower as clang 19's `EmitAArch64BuiltinExpr`: `vldN[q]_v` a call
// of `llvm.aarch64.neon.ldN` returning a literal struct of N vectors,
// stored to the result object; `vstN[q]_v` a call of
// `llvm.aarch64.neon.stN` with the vectors then the pointer; `vsli[q]_n_v`
// a call of `llvm.aarch64.neon.vsli` with an `i32` shift
// (spec/landed/sema/WP-799.md "NEON structure loads and stores"). WP-799.
//@ requires: clang<22
//@ requires: target-aarch64
//@ expect: ir
//@ args: -ftrivial-auto-var-init=uninitialized
#include <arm_neon.h>
uint16x8x2_t ld2q_u16(const uint16_t* p) { return vld2q_u16(p); }
uint8x8x2_t ld2_u8(const uint8_t* p) { return vld2_u8(p); }
uint8x8x3_t ld3_u8(const uint8_t* p) { return vld3_u8(p); }
int32x4x3_t ld3q_s32(const int32_t* p) { return vld3q_s32(p); }
float32x4x4_t ld4q_f32(const float* p) { return vld4q_f32(p); }
int16x4x4_t ld4_s16(const int16_t* p) { return vld4_s16(p); }
void st2q_u16(uint16_t* p, uint16x8x2_t v) { vst2q_u16(p, v); }
void st2_f32(float* p, float32x2x2_t v) { vst2_f32(p, v); }
void st3_u8(uint8_t* p, uint8x8x3_t v) { vst3_u8(p, v); }
void st3q_s64(int64_t* p, int64x2x3_t v) { vst3q_s64(p, v); }
void st4q_f32(float* p, float32x4x4_t v) { vst4q_f32(p, v); }
void st4_s32(int32_t* p, int32x2x4_t v) { vst4_s32(p, v); }
uint8x8_t sli_u8(uint8x8_t a, uint8x8_t b) { return vsli_n_u8(a, b, 4); }
uint16x8_t sliq_u16(uint16x8_t a, uint16x8_t b) { return vsliq_n_u16(a, b, 15); }
uint32x4_t sliq_u32(uint32x4_t a, uint32x4_t b) { return vsliq_n_u32(a, b, 0); }
int64x1_t sli_s64(int64x1_t a, int64x1_t b) { return vsli_n_s64(a, b, 63); }
poly8x16_t sliq_p8(poly8x16_t a, poly8x16_t b) { return vsliq_n_p8(a, b, 3); }
