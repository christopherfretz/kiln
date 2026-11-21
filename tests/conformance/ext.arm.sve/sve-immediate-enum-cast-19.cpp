// [ACLE] An immediate operand cast to an enumeration out of its range: at
// clang 19 `(svpattern)32` is an integer constant 32, outside the range
// [0, 31] of `svcntb_pat`'s operand (clang `SemaARM::
// CheckSVEBuiltinFunctionCall`). The clang 22 twin is
// sve-immediate-enum-cast.cpp. spec/sema.md "SVE and SME builtin calls"
// (WP-1792).
//@ requires: clang<22
//@ expect: compile-fail
//@ args: --target=aarch64-unknown-linux-gnu -march=armv8-a+sve
//@ strict-diags
//@ diag: 15:44 error "argument value 32 is outside the valid range [0, 31]"
typedef unsigned long uint64_t;
enum svpattern { SV_POW2 = 0, SV_ALL = 31 };
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svcntb_pat))) uint64_t svcntb_pat(enum svpattern);
uint64_t f() { return svcntb_pat(SV_ALL) + svcntb_pat((svpattern)32); }
