// [cpp.predefined]/2 (implementation-defined macros), Arm ACLE §2.4
// "Feature test macros": the SME and FP8 `-march=` modifiers (WP-1167).
// clang 19 defines no FP8 macro, turns on the FP16 arithmetic macros for
// `+sme` and SVE_B16B16 for `+sme-b16b16`; `+pauth-lr` also defines PAUTH.
// Both oracles (clang 22, and clang 19 at -fclang-compat=19) are matched;
// the target is pinned and the macros do not depend on the host.
// spec/pp.md "AArch64 target macros", spec/driver.md §Target features.
// The clang 19 output (clang 22 differs; sibling of the clang 22 test).
//@ requires: clang<22
//@ expect: pp
//@ args: --target=aarch64-unknown-linux-gnu -march=armv8-a+sme2p1+sme-f8f16+sme-b16b16+fp8dot2+ssve-fp8fma+pauth-lr
#include "arm-feature-macros.h"
