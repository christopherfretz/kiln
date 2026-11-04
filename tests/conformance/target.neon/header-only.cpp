// [driver] Including `<arm_neon.h>` without using it parses every
// intrinsic's inline definition (the bf16 ones call
// `__builtin_neon_vbfdotq_f32` and friends, declared whatever the target
// features; spec/landed/sema/WP-558.md "Target builtins"). WP-558.
//@ requires: target-aarch64
//@ strict-diags
#include <arm_neon.h>

int main() { return 0; }
