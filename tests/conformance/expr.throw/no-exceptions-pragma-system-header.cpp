// [expr.throw], [cpp.pragma]: with -fno-exceptions the throw-expressions
// of a header marked `#pragma GCC system_header` are not diagnosed, in a
// non-template function, a function template and a class template's
// members, also when instantiated from user code (clang's system-header
// rule). spec/sema.md "Exceptions disabled", WP-607.
//@ expect: compile-ok
//@ strict-diags
//@ args: -fno-exceptions
#include "Inputs/no_exc_pragma.h"

void use(int x) {
  sys_throw(x);
  sys_throw_t(x);
  SysThrow<int>().m();
  SysThrow<long>().n();
}
