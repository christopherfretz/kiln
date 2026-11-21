// [cpp.predefined]/2 (implementation-defined macros), Arm ACLE §2.4
// "Feature test macros": `-mgeneral-regs-only` turns `fp-armv8` off, and
// with it NEON and every extension that needs FP (here SVE, the crypto
// extensions and FP16 from `-march=`), so `__ARM_FP`, `__ARM_NEON`,
// `__ARM_NEON_FP` and the macros of those extensions drop out.
// Both oracles (clang 22, and clang 19 at -fclang-compat=19) agree here;
// the target is pinned and the macros do not depend on the host.
// spec/driver.md "AArch64 target features" (WP-1804).
//@ expect: pp
//@ args: --target=aarch64-unknown-linux-gnu -march=armv8.2-a+sve+crypto+fp16 -mgeneral-regs-only
#include "arm-feature-macros.h"
