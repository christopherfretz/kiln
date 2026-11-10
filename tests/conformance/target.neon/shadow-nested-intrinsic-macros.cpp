// [basic.scope.block]: the NEON intrinsics of <arm_neon.h> are macros
// whose bodies declare a `__ret` local; nesting them hides the outer
// `__ret`, which clang does not diagnose because -Wshadow is
// SuppressInSystemMacro (spec/diag.md §5.4). Reduced from the
// -Wshadow -Werror build of simdjson on aarch64 (WP-1227).
//@ requires: target-aarch64
//@ expect: compile-ok
//@ strict-diags
//@ args: -std=c++17 -Wshadow -Werror
#include <arm_neon.h>
uint64_t f(uint8x16_t v) {
  return vget_lane_u64(vreinterpret_u64_u8(vshrn_n_u16(vreinterpretq_u16_u8(v), 4)), 0);
}
int main() { return (int)f(vdupq_n_u8(0)); }
