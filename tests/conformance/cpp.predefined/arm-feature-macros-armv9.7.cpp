// [cpp.predefined]/2 (implementation-defined macros), Arm ACLE §2.4
// "Feature test macros": clang 22's newest level (clang 19 rejects it):
// SVE2p1/SVE2p2, FPRCVT, CSSC and the rest of its defaults.
// Both oracles (clang 22, and clang 19 at -fclang-compat=19) are matched;
// the target is pinned and the macros do not depend on the host.
// spec/pp.md "AArch64 target macros", spec/driver.md §Target features.
//@ expect: pp
//@ args: --target=aarch64-unknown-linux-gnu -march=armv9.7-a
//@ requires: clang>=22
#include "arm-feature-macros.h"
