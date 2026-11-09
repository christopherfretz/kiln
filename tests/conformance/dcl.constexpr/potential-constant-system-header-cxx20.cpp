// [dcl.constexpr]/6 (no diagnostic required): clang 19 skips the
// never-constant check for a function declared in a system header, even
// under -Wsystem-headers (`CheckConstexprFunctionBody`), so libstdc++'s
// <cmath> overloads calling `__builtin_acosf` are not reported (WP-945).
// spec/sema.md "Never constant".
//@ std: c++20
//@ strict-diags
//@ expect: compile-ok
//@ args: -Wsystem-headers
#include "Inputs/never_constant_sys.h"

float use(float x) { return sys_acos(x) + float(sys_floor()) + float(sys_asm(1)); }
