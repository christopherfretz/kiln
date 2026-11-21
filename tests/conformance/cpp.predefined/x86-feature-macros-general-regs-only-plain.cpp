// [cpp.predefined]/2 (implementation-defined macros): x86-64 target
// feature macros under `-mgeneral-regs-only` alone: no `__SSE__`,
// `__SSE2__`, `__MMX__`, `__SSE_MATH__` / `__SSE2_MATH__` and no
// `_Float16` macros (they need SSE2), as clang. The target is pinned and
// the macros do not depend on the host. spec/driver.md "x86-64 target
// features" (WP-1804).
//@ expect: pp
//@ args: --target=x86_64-unknown-linux-gnu -mgeneral-regs-only
#include "x86-feature-macros.h"
