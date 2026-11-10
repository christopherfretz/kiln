// [basic.scope.block]: with -Wsystem-headers the system-macro rule is off
// (clang's SuppressSystemWarnings; spec/diag.md §5.4) and the inner
// `__ret` of a nested system macro warns at the expansion.
//@ verify
//@ args: -Wshadow -Wsystem-headers -isystem %{test_dir}/Inputs
#include <shadow_system_macros.h>

int f(int v) {
  return SYS_OUTER(v); // expected-warning {{declaration shadows a local variable}} expected-note {{previous declaration is here}}
}
