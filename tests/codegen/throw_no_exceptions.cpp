//@ args: -fno-exceptions
// [expr.throw], [except.pre]: with -fno-exceptions a throw-expression or
// try-block that survives sema (in a system header, here instantiated)
// lowers as clang's: the `__cxa_allocate_exception` / `__cxa_throw`
// sequence with plain calls and no landing pads, and a try-block's
// compound statement with its handlers dropped. WP-607.
#include "Inputs/no_exc_system.h"

int use(int x) {
  sys_throw(x);
  return sys_try_throw(x) + sys_try_throw(2L);
}
