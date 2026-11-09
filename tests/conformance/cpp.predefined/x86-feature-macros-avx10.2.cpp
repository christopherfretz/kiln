// [cpp.predefined]/2 (implementation-defined macros): x86-64 target
// feature macros. `-mavx10.2`: the implied-feature closure, with -mno- also
// removing the features that depend on it, as clang's X86TargetParser.
// The target is pinned and the macros do not depend on the host.
// spec/driver.md "x86-64 target features" (WP-1000).
// clang 19 does not know the flag (an unknown argument).
//@ requires: clang>=22
//@ expect: pp
//@ args: --target=x86_64-unknown-linux-gnu -mavx10.2
#include "x86-feature-macros.h"
