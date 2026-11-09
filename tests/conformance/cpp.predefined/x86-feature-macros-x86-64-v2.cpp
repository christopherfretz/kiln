// [cpp.predefined]/2 (implementation-defined macros): x86-64 target
// feature macros. -march=x86-64-v2 is clang's CPU feature set for that CPU.
// The target is pinned and the macros do not depend on the host.
// spec/driver.md "x86-64 target features" (WP-1000).
// clang 19 and clang 22 agree.
//@ expect: pp
//@ args: --target=x86_64-unknown-linux-gnu -march=x86-64-v2
#include "x86-feature-macros.h"
