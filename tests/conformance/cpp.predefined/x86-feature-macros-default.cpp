// [cpp.predefined]/2 (implementation-defined macros): x86-64 target
// feature macros. the default x86-64 target (no -march=, no -m<feature>).
// The target is pinned and the macros do not depend on the host.
// spec/driver.md "x86-64 target features" (WP-1000).
// clang 19 and clang 22 agree.
//@ expect: pp
//@ args: --target=x86_64-unknown-linux-gnu
#include "x86-feature-macros.h"
