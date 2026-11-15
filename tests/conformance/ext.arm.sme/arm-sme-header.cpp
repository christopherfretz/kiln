// [ACLE] <arm_sme.h> compiles with the SME keyword attributes in place:
// its inline functions call `__builtin_arm_get_sme_state` and its SME
// intrinsics are `__clang_arm_builtin_alias` declarations of
// `__builtin_sme_*` builtins, which kiln does not provide yet (WP-1426
// follow-up). Host-restricted (no pinned `--target`): the header reaches
// <stdint.h>.
//@ requires: target-aarch64
//@ expect: compile-ok
//@ args: -march=armv9-a+sme2
//@ xfail(kiln): WP-1426 follow-up, the SME builtins of <arm_sme.h> (__builtin_arm_get_sme_state, __builtin_sme_*) are not provided
#include <arm_sme.h>
void f() __arm_streaming __arm_inout("za") { svzero_za(); }
bool g() __arm_streaming_compatible { return __arm_in_streaming_mode(); }
