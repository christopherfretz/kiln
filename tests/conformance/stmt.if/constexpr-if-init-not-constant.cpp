// [stmt.if]/2, [expr.const]: only the condition of a constexpr if is a
// manifestly constant-evaluated context; its init-statement is not. A
// non-constexpr variable declared there holding a reflection is outside
// an immediate function context ([expr.const.imm]); a constexpr one is
// fine. Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ args: -ferror-limit=0 -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ diag: 17:26 error "consteval-only value outside an immediate function context"
#include "kiln_meta.h"

int f() {
  if constexpr (constexpr auto r = ^^int; r == ^^int)
    return 1;
  if constexpr (auto r = ^^int; true)
    return 2;
  return 0;
}
