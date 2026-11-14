// [dcl.link]/6-7, [cpp.include]: a declaration with C language linkage
// in a namespace names the entity of the same name an included header
// declares with C language linkage in another namespace, and conflicts
// with a header's global variable of that name. With `--pch-diff`, the
// header's snapshot gives the same errors (WP-1293: a `load`
// consumer accepted both).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:31 error "redeclaration of 'xc_v' with a different type: 'long' vs 'int'"
//@ diag: 14:30 error "declaration of 'xc_g' with C language linkage conflicts with declaration in global scope"
#include "Inputs/extern_c.h"

namespace q { extern "C" long xc_v; }
namespace r { extern "C" int xc_g; }
namespace s { extern "C" int xc_f(int); }
int use() { return s::xc_f(xc_v); }
