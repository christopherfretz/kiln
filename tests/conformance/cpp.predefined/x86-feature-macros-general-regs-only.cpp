// [cpp.predefined]/2 (implementation-defined macros): x86-64 target
// feature macros. `-mgeneral-regs-only` is `-mno-x87 -mno-mmx -mno-sse`
// where it stands among the feature flags, so `-mavx2` before it is undone
// and `-msse2` after it turns SSE and SSE2 back on (clang
// `getX86TargetFeatures`). The target is pinned and the macros do not
// depend on the host. spec/driver.md "x86-64 target features" (WP-1804).
//@ expect: pp
//@ args: --target=x86_64-unknown-linux-gnu -mavx2 -mgeneral-regs-only -msse2
#include "x86-feature-macros.h"
