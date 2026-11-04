// [driver] The bf16 NEON builtins lower as clang 19's
// `EmitAArch64BuiltinExpr`: `bfdot` (result kept as `<N x float>`),
// `bfmlalb`/`bfmlalt`/`bfmmla`, `bfcvt`, `bfcvtn`/`bfcvtn2` (results
// bitcast to the builtin's declared type), declared
// `nocallback nofree nosync nounwind willreturn memory(none)`
// (spec/landed/codegen/WP-558.md "Target features"). WP-558.
//@ requires: clang<22
//@ requires: target-aarch64
//@ expect: ir
//@ args: -march=armv8.6-a+bf16 -ftrivial-auto-var-init=uninitialized
#include <arm_neon.h>

float32x2_t d2(float32x2_t r, bfloat16x4_t a, bfloat16x4_t b) { return vbfdot_f32(r, a, b); }
float32x4_t d4(float32x4_t r, bfloat16x8_t a, bfloat16x8_t b) { return vbfdotq_f32(r, a, b); }
float32x4_t mb(float32x4_t r, bfloat16x8_t a, bfloat16x8_t b) { return vbfmlalbq_f32(r, a, b); }
float32x4_t mt(float32x4_t r, bfloat16x8_t a, bfloat16x8_t b) { return vbfmlaltq_f32(r, a, b); }
float32x4_t mm(float32x4_t r, bfloat16x8_t a, bfloat16x8_t b) { return vbfmmlaq_f32(r, a, b); }
bfloat16_t c1(float32_t a) { return vcvth_bf16_f32(a); }
bfloat16x8_t c2(float32x4_t a) { return vcvtq_low_bf16_f32(a); }
bfloat16x8_t c3(bfloat16x8_t i, float32x4_t a) { return vcvtq_high_bf16_f32(i, a); }
bfloat16x4_t c4(float32x4_t a) { return vcvt_bf16_f32(a); }
