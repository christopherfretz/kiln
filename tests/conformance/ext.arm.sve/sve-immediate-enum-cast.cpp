// [ACLE] An immediate operand cast to an enumeration out of its range: at
// clang 22 `(svpattern)32` is no integer constant expression (the value is
// outside the range of `svpattern`'s values), so the call's operand "must
// be a constant integer" (clang `SemaARM::CheckSVEBuiltinFunctionCall`).
// The clang 19 twin is sve-immediate-enum-cast-19.cpp. spec/sema.md "SVE
// and SME builtin calls" (WP-1792).
//@ requires: clang>=22
//@ expect: compile-fail
//@ args: --target=aarch64-unknown-linux-gnu -march=armv8-a+sve
//@ strict-diags
//@ diag: 15:44 error "argument to 'svcntb_pat' must be a constant integer"
typedef unsigned long uint64_t;
enum svpattern { SV_POW2 = 0, SV_ALL = 31 };
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svcntb_pat))) uint64_t svcntb_pat(enum svpattern);
uint64_t f() { return svcntb_pat(SV_ALL) + svcntb_pat((svpattern)32); }
