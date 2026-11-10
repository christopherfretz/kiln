// [cpp.predefined]/2 (implementation-defined macros), Arm ACLE §2.4
// "Feature test macros": the SME, FP8 and newer `-march=` modifiers
// (WP-1167; kiln rejected `-march=armv9-a+sme2` before its generated
// modifier table) define their macros, and `+pauth-lr` also defines PAUTH.
// Both oracles (clang 22, and clang 19 at -fclang-compat=19) are matched;
// the target is pinned and the macros do not depend on the host.
// spec/pp.md "AArch64 target macros", spec/driver.md §Target features.
//@ requires: clang>=22
//@ expect: pp
//@ args: --target=aarch64-unknown-linux-gnu -march=armv8-a+sme2p2+sme-f8f16+sme-b16b16+fp8dot2+ssve-fp8fma+ssve-aes+pauth-lr+sve-bfscale+f8f32mm
#include "arm-feature-macros.h"
