// [driver] Outside `-fms-extensions` (which kiln++ does not accept) the
// `arm_acle.h` barrier and hint intrinsics `__dmb`/`__dsb`/`__isb` and
// `__yield`/`__wfe`/`__wfi`/`__sev`/`__sevl` are the header's inline
// functions over `__builtin_arm_dmb`/`dsb`/`isb` and `__builtin_arm_hint`,
// since `__has_builtin` denies the MS builtins of the same names, as
// clang's (spec/pp.md "Feature queries"). WP-846.
//@ requires: target-aarch64
//@ strict-diags
#include <arm_acle.h>
void f() {
  __dmb(11);
  __dsb(15);
  __isb(15);
  __yield();
  __wfe();
  __wfi();
  __sev();
  __sevl();
}
