// [cpp.predefined]/2 (implementation-defined macros), Arm ACLE §2.4
// "Feature test macros": +crypto from v8.4 also enables SHA3/SHA512 and
// SM3/SM4; the v8.1-v8.4 level macros (QRDMX, COMPLEX, JCVT, RCPC,
// DOTPROD).
// Both oracles (clang 22, and clang 19 at -fclang-compat=19) are matched;
// the target is pinned and the macros do not depend on the host.
// spec/pp.md "AArch64 target macros", spec/driver.md §Target features.
//@ requires: clang>=22
//@ expect: pp
//@ args: --target=aarch64-unknown-linux-gnu -march=armv8.4-a+crypto
#include "arm-feature-macros.h"
