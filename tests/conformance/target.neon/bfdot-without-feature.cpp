// [driver] Calling the bf16 intrinsic `vbfdotq_f32` from a function
// compiled without bf16 is accepted by sema: clang's
// `err_function_needs_feature` ("always_inline function 'vbfdotq_f32'
// requires target feature 'bf16', but would be inlined into function
// 'f' ...") is a codegen error, pinned against clang 19 by the driver
// test `neon_bf16_call_without_feature` (spec/codegen.md "Target
// features"). WP-558.
//@ requires: target-aarch64
//@ strict-diags
#include <arm_neon.h>

float32x4_t f(float32x4_t r, bfloat16x8_t a, bfloat16x8_t b) { return vbfdotq_f32(r, a, b); }
