// [cpp.predefined]/2 (implementation-defined macros), Arm ACLE §2.4
// "Feature test macros": the default armv8-a target has FP and NEON and
// none of the optional feature macros.
// Both oracles (clang 22, and clang 19 at -fclang-compat=19) are matched;
// the target is pinned and the macros do not depend on the host.
// spec/pp.md "AArch64 target macros", spec/driver.md §Target features.
//@ expect: pp
//@ args: --target=aarch64-unknown-linux-gnu 
#include "arm-feature-macros.h"
