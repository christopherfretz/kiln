// [driver] `vpaddl[q]` lowers as clang 19's `EmitCommonNeonBuiltinExpr`:
// `{s,u}addlp.VTy.N` of the operand as the vector of twice the lanes at
// half the width; `vqdmulh[q]` and `vqrdmulh[q]` one `sq[r]dmulh` call
// each (simdjson-style UTF-8 validation in daw_json_link, glaze; WP-792).
//@ requires: clang<22
//@ requires: target-aarch64
//@ expect: ir
//@ args: -ftrivial-auto-var-init=uninitialized
#include <arm_neon.h>
auto f_vpaddl_s8(int8x8_t a) { return vpaddl_s8(a); }
auto f_vpaddl_u8(uint8x8_t a) { return vpaddl_u8(a); }
auto f_vpaddl_s16(int16x4_t a) { return vpaddl_s16(a); }
auto f_vpaddl_u16(uint16x4_t a) { return vpaddl_u16(a); }
auto f_vpaddl_s32(int32x2_t a) { return vpaddl_s32(a); }
auto f_vpaddl_u32(uint32x2_t a) { return vpaddl_u32(a); }
auto f_vpaddlq_s8(int8x16_t a) { return vpaddlq_s8(a); }
auto f_vpaddlq_u8(uint8x16_t a) { return vpaddlq_u8(a); }
auto f_vpaddlq_s16(int16x8_t a) { return vpaddlq_s16(a); }
auto f_vpaddlq_u16(uint16x8_t a) { return vpaddlq_u16(a); }
auto f_vpaddlq_s32(int32x4_t a) { return vpaddlq_s32(a); }
auto f_vpaddlq_u32(uint32x4_t a) { return vpaddlq_u32(a); }
auto f_vqdmulh_s16(int16x4_t a, int16x4_t b) { return vqdmulh_s16(a, b); }
auto f_vqdmulh_s32(int32x2_t a, int32x2_t b) { return vqdmulh_s32(a, b); }
auto f_vqdmulhq_s16(int16x8_t a, int16x8_t b) { return vqdmulhq_s16(a, b); }
auto f_vqdmulhq_s32(int32x4_t a, int32x4_t b) { return vqdmulhq_s32(a, b); }
auto f_vqrdmulh_s16(int16x4_t a, int16x4_t b) { return vqrdmulh_s16(a, b); }
auto f_vqrdmulh_s32(int32x2_t a, int32x2_t b) { return vqrdmulh_s32(a, b); }
auto f_vqrdmulhq_s16(int16x8_t a, int16x8_t b) { return vqrdmulhq_s16(a, b); }
auto f_vqrdmulhq_s32(int32x4_t a, int32x4_t b) { return vqrdmulhq_s32(a, b); }
