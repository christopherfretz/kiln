// [cpp.predefined]/2 (implementation-defined macros): x86-64 target
// feature macros. `-msse4.2 -mavx2 -mpclmul -msha -mno-avx512f`: the implied-feature closure, with -mno- also
// removing the features that depend on it, as clang's X86TargetParser.
// The target is pinned and the macros do not depend on the host.
// spec/driver.md "x86-64 target features" (WP-1000).
// clang 19 and clang 22 agree.
//@ expect: pp
//@ args: --target=x86_64-unknown-linux-gnu -msse4.2 -mavx2 -mpclmul -msha -mno-avx512f
#include "x86-feature-macros.h"
