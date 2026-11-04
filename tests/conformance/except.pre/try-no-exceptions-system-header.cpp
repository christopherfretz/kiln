// [except.pre], [cpp.include]: with -fno-exceptions a try-block in a
// system header -- found through -isystem, or after `#pragma GCC
// system_header` -- is not diagnosed (clang `ActOnCXXTryBlock`), in a
// non-template function and in instantiated templates. spec/sema.md
// "Exceptions disabled", WP-607.
//@ expect: compile-ok
//@ strict-diags
//@ args: -fno-exceptions -isystem %{test_dir}/Inputs/isystem
#include <no_exc_try_isystem.h>
#include "Inputs/no_exc_try_pragma.h"

int use(int x) {
  return isys_try(x) + IsysTry<int>().get(x) + sys_try(x) + sys_try_t(x);
}
