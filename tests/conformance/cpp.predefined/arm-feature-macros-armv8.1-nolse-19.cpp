// [cpp.predefined]/2 (implementation-defined macros), Arm ACLE §2.4
// "Feature test macros": clang 19 ties ATOMICS/CRC32 to the v8.1 level
// whatever +nolse says; clang 22 follows the features.
// Both oracles (clang 22, and clang 19 at -fclang-compat=19) are matched;
// the target is pinned and the macros do not depend on the host.
// spec/pp.md "AArch64 target macros", spec/driver.md §Target features.
// The clang 19 output (clang 22 differs; sibling of the clang 22 test).
//@ requires: clang<22
//@ expect: pp
//@ args: --target=aarch64-unknown-linux-gnu -march=armv8.1-a+nolse
#include "arm-feature-macros.h"
