// [cpp.predefined]/2 (implementation-defined macros), Arm ACLE §2.4
// "Feature test macros": the SVE2 crypto extensions (clang 22 splits each
// into sve2 + sve-x).
// Both oracles (clang 22, and clang 19 at -fclang-compat=19) are matched;
// the target is pinned and the macros do not depend on the host.
// spec/pp.md "AArch64 target macros", spec/driver.md §Target features.
//@ expect: pp
//@ args: --target=aarch64-unknown-linux-gnu -march=armv9.2-a+crypto+sve2-aes+sve2-bitperm
#include "arm-feature-macros.h"
