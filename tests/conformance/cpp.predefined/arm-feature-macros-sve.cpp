// [cpp.predefined]/2 (implementation-defined macros), Arm ACLE §2.4
// "Feature test macros": +sve adds __ARM_FEATURE_SVE, the vector-operator
// level 2 and (through fullfp16) the FP16 arithmetic macros.
// Both oracles (clang 22, and clang 19 at -fclang-compat=19) are matched;
// the target is pinned and the macros do not depend on the host.
// spec/pp.md "AArch64 target macros", spec/driver.md §Target features.
//@ expect: pp
//@ args: --target=aarch64-unknown-linux-gnu -march=armv8-a+sve
#include "arm-feature-macros.h"
