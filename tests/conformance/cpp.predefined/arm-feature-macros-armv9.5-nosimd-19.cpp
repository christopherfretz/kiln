// [cpp.predefined]/2 (implementation-defined macros), Arm ACLE §2.4
// "Feature test macros": without NEON there is no SVE mode: SVE, the FP16
// vector macro and the NEON macros drop out.
// Both oracles (clang 22, and clang 19 at -fclang-compat=19) are matched;
// the target is pinned and the macros do not depend on the host.
// spec/pp.md "AArch64 target macros", spec/driver.md §Target features.
// The clang 19 output (clang 22 differs; sibling of the clang 22 test).
//@ requires: clang<22
//@ expect: pp
//@ args: --target=aarch64-unknown-linux-gnu -march=armv9.5-a+nosimd
#include "arm-feature-macros.h"
