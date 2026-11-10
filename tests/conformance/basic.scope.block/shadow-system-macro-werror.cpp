// [basic.scope.block]: nested system-header macros that each declare a
// `__ret` local (the shape of the <arm_neon.h> intrinsics) compile clean
// under -Wshadow -Werror: -Wshadow is SuppressInSystemMacro (spec/diag.md
// §5.4), and -Werror does not resurrect it. -Wsystem-headers turns the
// suppression off (see shadow-system-macro-wsystem-headers.cpp).
//@ expect: compile-ok
//@ strict-diags
//@ args: -Wshadow -Werror -isystem %{test_dir}/Inputs
#include <shadow_system_macros.h>

int f(int v) {
  int ab = 0;
  { SYS_PASTE_DECL(a, b); }
  return SYS_OUTER(SYS_OUTER(v)) + ab;
}
