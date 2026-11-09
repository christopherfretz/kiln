// [cpp.predefined]/2 (implementation-defined macros): x86-64 target
// feature macros. `-mapxf`: the implied-feature closure, with -mno- also
// removing the features that depend on it, as clang's X86TargetParser.
// The target is pinned and the macros do not depend on the host.
// spec/driver.md "x86-64 target features" (WP-1000).
// The clang 19 output (clang 22 differs; sibling of the clang 22 test).
//@ requires: clang<22
//@ expect: pp
//@ args: --target=x86_64-unknown-linux-gnu -mapxf
#include "x86-feature-macros.h"
