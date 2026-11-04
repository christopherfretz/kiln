// [driver] The NEON lane, splat, narrowing and permute builtins lower as
// clang 19's `EmitAArch64BuiltinExpr` / `EmitCommonNeonBuiltinExpr`:
// `vget[q]_lane_*`/`vdup<b|h|s|d>_lane[q]_*` an `extractelement`,
// `vset[q]_lane_*` an `insertelement`, `splat[q]_lane[q]_v` a splat
// `shufflevector`, `vmovn_v` a `trunc`, `vext[q]_v` a sliding
// `shufflevector`, `vzip`/`vuzp`/`vtrn[q]_v` two shuffles stored through
// the result pointer, `vqtbl1[q]_v` `llvm.aarch64.neon.tbl1`, `vshr[q]_n_v`
// an `lshr`/`ashr` by a splat (zero, or width - 1, at the full width)
// (spec/landed/codegen/WP-628.md
// "NEON load/store builtins"). WP-628.
//@ requires: clang<22
//@ requires: target-aarch64
//@ expect: ir
//@ args: -ftrivial-auto-var-init=uninitialized
#include <arm_neon.h>
uint8_t get_u8(uint8x8_t v) { return vget_lane_u8(v, 7); }
uint8_t getq_u8(uint8x16_t v) { return vgetq_lane_u8(v, 15); }
int16_t get_s16(int16x4_t v) { return vget_lane_s16(v, 3); }
int16_t getq_s16(int16x8_t v) { return vgetq_lane_s16(v, 7); }
float16_t get_f16(float16x4_t v) { return vget_lane_f16(v, 3); }
float16_t getq_f16(float16x8_t v) { return vgetq_lane_f16(v, 7); }
int32_t get_s32(int32x2_t v) { return vget_lane_s32(v, 1); }
int32_t getq_s32(int32x4_t v) { return vgetq_lane_s32(v, 3); }
float32_t get_f32(float32x2_t v) { return vget_lane_f32(v, 1); }
float32_t getq_f32(float32x4_t v) { return vgetq_lane_f32(v, 3); }
uint64_t get_u64(uint64x1_t v) { return vget_lane_u64(v, 0); }
uint64_t getq_u64(uint64x2_t v) { return vgetq_lane_u64(v, 1); }
poly64_t get_p64(poly64x1_t v) { return vget_lane_p64(v, 0); }
poly64_t getq_p64(poly64x2_t v) { return vgetq_lane_p64(v, 1); }
float64_t get_f64(float64x1_t v) { return vget_lane_f64(v, 0); }
float64_t getq_f64(float64x2_t v) { return vgetq_lane_f64(v, 1); }
uint8x8_t set_u8(uint8_t x, uint8x8_t v) { return vset_lane_u8(x, v, 7); }
uint8x16_t setq_u8(uint8_t x, uint8x16_t v) { return vsetq_lane_u8(x, v, 15); }
int8x8_t set_s8(int8_t x, int8x8_t v) { return vset_lane_s8(x, v, 7); }
int8x16_t setq_s8(int8_t x, int8x16_t v) { return vsetq_lane_s8(x, v, 15); }
poly8x8_t set_p8(poly8_t x, poly8x8_t v) { return vset_lane_p8(x, v, 7); }
poly8x16_t setq_p8(poly8_t x, poly8x16_t v) { return vsetq_lane_p8(x, v, 15); }
int16x4_t set_s16(int16_t x, int16x4_t v) { return vset_lane_s16(x, v, 3); }
int16x8_t setq_s16(int16_t x, int16x8_t v) { return vsetq_lane_s16(x, v, 7); }
float16x4_t set_f16(float16_t x, float16x4_t v) { return vset_lane_f16(x, v, 3); }
float16x8_t setq_f16(float16_t x, float16x8_t v) { return vsetq_lane_f16(x, v, 7); }
uint32x2_t set_u32(uint32_t x, uint32x2_t v) { return vset_lane_u32(x, v, 1); }
uint32x4_t setq_u32(uint32_t x, uint32x4_t v) { return vsetq_lane_u32(x, v, 3); }
float32x2_t set_f32(float32_t x, float32x2_t v) { return vset_lane_f32(x, v, 1); }
float32x4_t setq_f32(float32_t x, float32x4_t v) { return vsetq_lane_f32(x, v, 3); }
uint64x1_t set_u64(uint64_t x, uint64x1_t v) { return vset_lane_u64(x, v, 0); }
uint64x2_t setq_u64(uint64_t x, uint64x2_t v) { return vsetq_lane_u64(x, v, 1); }
int64x1_t set_s64(int64_t x, int64x1_t v) { return vset_lane_s64(x, v, 0); }
int64x2_t setq_s64(int64_t x, int64x2_t v) { return vsetq_lane_s64(x, v, 1); }
float64x1_t set_f64(float64_t x, float64x1_t v) { return vset_lane_f64(x, v, 0); }
float64x2_t setq_f64(float64_t x, float64x2_t v) { return vsetq_lane_f64(x, v, 1); }
uint8x8_t dupl_u8(uint8x8_t v) { return vdup_lane_u8(v, 7); }
uint8x8_t duplq_u8(uint8x16_t v) { return vdup_laneq_u8(v, 15); }
uint8x16_t duplq_u8(uint8x8_t v) { return vdupq_lane_u8(v, 7); }
uint8x16_t duplqq_u8(uint8x16_t v) { return vdupq_laneq_u8(v, 15); }
int8x8_t dupl_s8(int8x8_t v) { return vdup_lane_s8(v, 7); }
int8x8_t duplq_s8(int8x16_t v) { return vdup_laneq_s8(v, 15); }
int8x16_t duplq_s8(int8x8_t v) { return vdupq_lane_s8(v, 7); }
int8x16_t duplqq_s8(int8x16_t v) { return vdupq_laneq_s8(v, 15); }
poly8x8_t dupl_p8(poly8x8_t v) { return vdup_lane_p8(v, 7); }
poly8x8_t duplq_p8(poly8x16_t v) { return vdup_laneq_p8(v, 15); }
poly8x16_t duplq_p8(poly8x8_t v) { return vdupq_lane_p8(v, 7); }
poly8x16_t duplqq_p8(poly8x16_t v) { return vdupq_laneq_p8(v, 15); }
int16x4_t dupl_s16(int16x4_t v) { return vdup_lane_s16(v, 3); }
int16x4_t duplq_s16(int16x8_t v) { return vdup_laneq_s16(v, 7); }
int16x8_t duplq_s16(int16x4_t v) { return vdupq_lane_s16(v, 3); }
int16x8_t duplqq_s16(int16x8_t v) { return vdupq_laneq_s16(v, 7); }
float16x4_t dupl_f16(float16x4_t v) { return vdup_lane_f16(v, 3); }
float16x4_t duplq_f16(float16x8_t v) { return vdup_laneq_f16(v, 7); }
float16x8_t duplq_f16(float16x4_t v) { return vdupq_lane_f16(v, 3); }
float16x8_t duplqq_f16(float16x8_t v) { return vdupq_laneq_f16(v, 7); }
uint32x2_t dupl_u32(uint32x2_t v) { return vdup_lane_u32(v, 1); }
uint32x2_t duplq_u32(uint32x4_t v) { return vdup_laneq_u32(v, 3); }
uint32x4_t duplq_u32(uint32x2_t v) { return vdupq_lane_u32(v, 1); }
uint32x4_t duplqq_u32(uint32x4_t v) { return vdupq_laneq_u32(v, 3); }
float32x2_t dupl_f32(float32x2_t v) { return vdup_lane_f32(v, 1); }
float32x2_t duplq_f32(float32x4_t v) { return vdup_laneq_f32(v, 3); }
float32x4_t duplq_f32(float32x2_t v) { return vdupq_lane_f32(v, 1); }
float32x4_t duplqq_f32(float32x4_t v) { return vdupq_laneq_f32(v, 3); }
uint64x1_t dupl_u64(uint64x1_t v) { return vdup_lane_u64(v, 0); }
uint64x1_t duplq_u64(uint64x2_t v) { return vdup_laneq_u64(v, 1); }
uint64x2_t duplq_u64(uint64x1_t v) { return vdupq_lane_u64(v, 0); }
uint64x2_t duplqq_u64(uint64x2_t v) { return vdupq_laneq_u64(v, 1); }
int64x1_t dupl_s64(int64x1_t v) { return vdup_lane_s64(v, 0); }
int64x1_t duplq_s64(int64x2_t v) { return vdup_laneq_s64(v, 1); }
int64x2_t duplq_s64(int64x1_t v) { return vdupq_lane_s64(v, 0); }
int64x2_t duplqq_s64(int64x2_t v) { return vdupq_laneq_s64(v, 1); }
float64x1_t dupl_f64(float64x1_t v) { return vdup_lane_f64(v, 0); }
float64x1_t duplq_f64(float64x2_t v) { return vdup_laneq_f64(v, 1); }
float64x2_t duplq_f64(float64x1_t v) { return vdupq_lane_f64(v, 0); }
float64x2_t duplqq_f64(float64x2_t v) { return vdupq_laneq_f64(v, 1); }
uint8x8_t ext_u8(uint8x8_t a, uint8x8_t b) { return vext_u8(a, b, 7); }
uint8x16_t extq_u8(uint8x16_t a, uint8x16_t b) { return vextq_u8(a, b, 15); }
int8x8_t ext_s8(int8x8_t a, int8x8_t b) { return vext_s8(a, b, 7); }
int8x16_t extq_s8(int8x16_t a, int8x16_t b) { return vextq_s8(a, b, 15); }
poly8x8_t ext_p8(poly8x8_t a, poly8x8_t b) { return vext_p8(a, b, 7); }
poly8x16_t extq_p8(poly8x16_t a, poly8x16_t b) { return vextq_p8(a, b, 15); }
int16x4_t ext_s16(int16x4_t a, int16x4_t b) { return vext_s16(a, b, 3); }
int16x8_t extq_s16(int16x8_t a, int16x8_t b) { return vextq_s16(a, b, 7); }
float16x4_t ext_f16(float16x4_t a, float16x4_t b) { return vext_f16(a, b, 3); }
float16x8_t extq_f16(float16x8_t a, float16x8_t b) { return vextq_f16(a, b, 7); }
uint32x2_t ext_u32(uint32x2_t a, uint32x2_t b) { return vext_u32(a, b, 1); }
uint32x4_t extq_u32(uint32x4_t a, uint32x4_t b) { return vextq_u32(a, b, 3); }
float32x2_t ext_f32(float32x2_t a, float32x2_t b) { return vext_f32(a, b, 1); }
float32x4_t extq_f32(float32x4_t a, float32x4_t b) { return vextq_f32(a, b, 3); }
uint64x1_t ext_u64(uint64x1_t a, uint64x1_t b) { return vext_u64(a, b, 0); }
uint64x2_t extq_u64(uint64x2_t a, uint64x2_t b) { return vextq_u64(a, b, 1); }
int64x1_t ext_s64(int64x1_t a, int64x1_t b) { return vext_s64(a, b, 0); }
int64x2_t extq_s64(int64x2_t a, int64x2_t b) { return vextq_s64(a, b, 1); }
float64x1_t ext_f64(float64x1_t a, float64x1_t b) { return vext_f64(a, b, 0); }
float64x2_t extq_f64(float64x2_t a, float64x2_t b) { return vextq_f64(a, b, 1); }
uint8x8x2_t zip_u8(uint8x8_t a, uint8x8_t b) { return vzip_u8(a, b); }
uint8x8x2_t uzp_u8(uint8x8_t a, uint8x8_t b) { return vuzp_u8(a, b); }
uint8x8x2_t trn_u8(uint8x8_t a, uint8x8_t b) { return vtrn_u8(a, b); }
uint8x16x2_t zipq_u8(uint8x16_t a, uint8x16_t b) { return vzipq_u8(a, b); }
uint8x16x2_t uzpq_u8(uint8x16_t a, uint8x16_t b) { return vuzpq_u8(a, b); }
uint8x16x2_t trnq_u8(uint8x16_t a, uint8x16_t b) { return vtrnq_u8(a, b); }
int8x8x2_t zip_s8(int8x8_t a, int8x8_t b) { return vzip_s8(a, b); }
int8x8x2_t uzp_s8(int8x8_t a, int8x8_t b) { return vuzp_s8(a, b); }
int8x8x2_t trn_s8(int8x8_t a, int8x8_t b) { return vtrn_s8(a, b); }
int8x16x2_t zipq_s8(int8x16_t a, int8x16_t b) { return vzipq_s8(a, b); }
int8x16x2_t uzpq_s8(int8x16_t a, int8x16_t b) { return vuzpq_s8(a, b); }
int8x16x2_t trnq_s8(int8x16_t a, int8x16_t b) { return vtrnq_s8(a, b); }
poly8x8x2_t zip_p8(poly8x8_t a, poly8x8_t b) { return vzip_p8(a, b); }
poly8x8x2_t uzp_p8(poly8x8_t a, poly8x8_t b) { return vuzp_p8(a, b); }
poly8x8x2_t trn_p8(poly8x8_t a, poly8x8_t b) { return vtrn_p8(a, b); }
poly8x16x2_t zipq_p8(poly8x16_t a, poly8x16_t b) { return vzipq_p8(a, b); }
poly8x16x2_t uzpq_p8(poly8x16_t a, poly8x16_t b) { return vuzpq_p8(a, b); }
poly8x16x2_t trnq_p8(poly8x16_t a, poly8x16_t b) { return vtrnq_p8(a, b); }
int16x4x2_t zip_s16(int16x4_t a, int16x4_t b) { return vzip_s16(a, b); }
int16x4x2_t uzp_s16(int16x4_t a, int16x4_t b) { return vuzp_s16(a, b); }
int16x4x2_t trn_s16(int16x4_t a, int16x4_t b) { return vtrn_s16(a, b); }
int16x8x2_t zipq_s16(int16x8_t a, int16x8_t b) { return vzipq_s16(a, b); }
int16x8x2_t uzpq_s16(int16x8_t a, int16x8_t b) { return vuzpq_s16(a, b); }
int16x8x2_t trnq_s16(int16x8_t a, int16x8_t b) { return vtrnq_s16(a, b); }
float16x4x2_t zip_f16(float16x4_t a, float16x4_t b) { return vzip_f16(a, b); }
float16x4x2_t uzp_f16(float16x4_t a, float16x4_t b) { return vuzp_f16(a, b); }
float16x4x2_t trn_f16(float16x4_t a, float16x4_t b) { return vtrn_f16(a, b); }
float16x8x2_t zipq_f16(float16x8_t a, float16x8_t b) { return vzipq_f16(a, b); }
float16x8x2_t uzpq_f16(float16x8_t a, float16x8_t b) { return vuzpq_f16(a, b); }
float16x8x2_t trnq_f16(float16x8_t a, float16x8_t b) { return vtrnq_f16(a, b); }
uint32x2x2_t zip_u32(uint32x2_t a, uint32x2_t b) { return vzip_u32(a, b); }
uint32x2x2_t uzp_u32(uint32x2_t a, uint32x2_t b) { return vuzp_u32(a, b); }
uint32x2x2_t trn_u32(uint32x2_t a, uint32x2_t b) { return vtrn_u32(a, b); }
uint32x4x2_t zipq_u32(uint32x4_t a, uint32x4_t b) { return vzipq_u32(a, b); }
uint32x4x2_t uzpq_u32(uint32x4_t a, uint32x4_t b) { return vuzpq_u32(a, b); }
uint32x4x2_t trnq_u32(uint32x4_t a, uint32x4_t b) { return vtrnq_u32(a, b); }
float32x2x2_t zip_f32(float32x2_t a, float32x2_t b) { return vzip_f32(a, b); }
float32x2x2_t uzp_f32(float32x2_t a, float32x2_t b) { return vuzp_f32(a, b); }
float32x2x2_t trn_f32(float32x2_t a, float32x2_t b) { return vtrn_f32(a, b); }
float32x4x2_t zipq_f32(float32x4_t a, float32x4_t b) { return vzipq_f32(a, b); }
float32x4x2_t uzpq_f32(float32x4_t a, float32x4_t b) { return vuzpq_f32(a, b); }
float32x4x2_t trnq_f32(float32x4_t a, float32x4_t b) { return vtrnq_f32(a, b); }
uint8x8_t shr_u8(uint8x8_t a) { return vshr_n_u8(a, 1); }
uint8x8_t shrw_u8(uint8x8_t a) { return vshr_n_u8(a, 8); }
uint8x16_t shrq_u8(uint8x16_t a) { return vshrq_n_u8(a, 1); }
uint8x16_t shrwq_u8(uint8x16_t a) { return vshrq_n_u8(a, 8); }
int8x8_t shr_s8(int8x8_t a) { return vshr_n_s8(a, 1); }
int8x8_t shrw_s8(int8x8_t a) { return vshr_n_s8(a, 8); }
int8x16_t shrq_s8(int8x16_t a) { return vshrq_n_s8(a, 1); }
int8x16_t shrwq_s8(int8x16_t a) { return vshrq_n_s8(a, 8); }
int16x4_t shr_s16(int16x4_t a) { return vshr_n_s16(a, 1); }
int16x4_t shrw_s16(int16x4_t a) { return vshr_n_s16(a, 16); }
int16x8_t shrq_s16(int16x8_t a) { return vshrq_n_s16(a, 1); }
int16x8_t shrwq_s16(int16x8_t a) { return vshrq_n_s16(a, 16); }
uint32x2_t shr_u32(uint32x2_t a) { return vshr_n_u32(a, 1); }
uint32x2_t shrw_u32(uint32x2_t a) { return vshr_n_u32(a, 32); }
uint32x4_t shrq_u32(uint32x4_t a) { return vshrq_n_u32(a, 1); }
uint32x4_t shrwq_u32(uint32x4_t a) { return vshrq_n_u32(a, 32); }
uint64x1_t shr_u64(uint64x1_t a) { return vshr_n_u64(a, 1); }
uint64x1_t shrw_u64(uint64x1_t a) { return vshr_n_u64(a, 64); }
uint64x2_t shrq_u64(uint64x2_t a) { return vshrq_n_u64(a, 1); }
uint64x2_t shrwq_u64(uint64x2_t a) { return vshrq_n_u64(a, 64); }
int64x1_t shr_s64(int64x1_t a) { return vshr_n_s64(a, 1); }
int64x1_t shrw_s64(int64x1_t a) { return vshr_n_s64(a, 64); }
int64x2_t shrq_s64(int64x2_t a) { return vshrq_n_s64(a, 1); }
int64x2_t shrwq_s64(int64x2_t a) { return vshrq_n_s64(a, 64); }
uint8x8_t tbl_u8(uint8x16_t t, uint8x8_t i) { return vqtbl1_u8(t, (uint8x8_t)i); }
uint8x16_t tblq_u8(uint8x16_t t, uint8x16_t i) { return vqtbl1q_u8(t, (uint8x16_t)i); }
int8x8_t tbl_s8(int8x16_t t, int8x8_t i) { return vqtbl1_s8(t, (uint8x8_t)i); }
int8x16_t tblq_s8(int8x16_t t, int8x16_t i) { return vqtbl1q_s8(t, (uint8x16_t)i); }
poly8x8_t tbl_p8(poly8x16_t t, poly8x8_t i) { return vqtbl1_p8(t, (uint8x8_t)i); }
poly8x16_t tblq_p8(poly8x16_t t, poly8x16_t i) { return vqtbl1q_p8(t, (uint8x16_t)i); }
uint8x8_t movn16(uint16x8_t v) { return vmovn_u16(v); }
int16x4_t movn32(int32x4_t v) { return vmovn_s32(v); }
uint32x2_t movn64(uint64x2_t v) { return vmovn_u64(v); }
uint8x16_t movn_high(uint8x8_t lo, uint16x8_t v) { return vmovn_high_u16(lo, v); }
uint8x16_t combine(uint8x8_t a, uint8x8_t b) { return vcombine_u8(a, b); }
float16_t duph(float16x4_t v) { return vduph_lane_f16(v, 3); }
uint8_t dupb(uint8x16_t v) { return vdupb_laneq_u8(v, 15); }
double dupd(float64x1_t v) { return vdupd_lane_f64(v, 0); }
