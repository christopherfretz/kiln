// [driver] `<arm_neon.h>` declares every NEON intrinsic, including the
// bf16 ones fenced only by `target("bf16,neon")`; a TU that includes it
// and calls a plain intrinsic compiles without a diagnostic. clang
// declares the `__builtin_neon_*` it calls unconditionally (spec/
// sema-builtins.md "Target builtins"). WP-558.
//@ requires: target-aarch64
//@ strict-diags
#include <arm_neon.h>

float32x4_t add(float32x4_t a, float32x4_t b) { return vaddq_f32(a, b); }
