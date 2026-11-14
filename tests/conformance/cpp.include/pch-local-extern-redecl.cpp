// [basic.link]/6, [basic.link]/11, [over.load]/2, [cpp.include]: a
// namespace-scope declaration redeclares the entity an included header's
// block-scope declaration names, so it must agree with it; a `static` one
// follows the non-static one ([dcl.stc]/7). With `--pch-diff`, the
// header's snapshot gives the same errors (WP-1293).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:6 error "functions that differ only in their return type cannot be overloaded"
//@ diag: 17:12 error "static declaration of 'le_var' follows non-static declaration"
//@ diag: 19:6 error "functions that differ only in their return type cannot be overloaded"
//@ diag: 26:6 error "functions that differ only in their return type cannot be overloaded"
//@ diag: 27:5 error "of 'le_global' with a different type: 'int' vs 'long'"
#include "Inputs/local_extern.h"

namespace le {
long le_fn(int);
static int le_var;
inline namespace in {
long le_inner();
}
// A different namespace: no conflict.
namespace other {
long le_fn(int);
}
}
long le_defined(int);
int le_global;
