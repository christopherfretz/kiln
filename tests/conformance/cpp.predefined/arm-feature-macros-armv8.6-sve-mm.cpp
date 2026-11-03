// [cpp.predefined]/2 (implementation-defined macros), Arm ACLE §2.4
// "Feature test macros": v8.6's BF16 and MATMUL_INT8 with their SVE forms,
// and SVE_MATMUL_FP64.
// Both oracles (clang 22, and clang 19 at -fclang-compat=19) are matched;
// the target is pinned and the macros do not depend on the host.
// spec/pp.md "AArch64 target macros", spec/driver.md §Target features.
//@ expect: pp
//@ args: --target=aarch64-unknown-linux-gnu -march=armv8.6-a+sve+f64mm
#include "arm-feature-macros.h"
