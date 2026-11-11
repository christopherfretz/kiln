// [basic.link]/6, [basic.link]/11, [cpp.include]: a block-scope
// declaration in the test file redeclares the entity an included header's
// block-scope declaration names, so the types must agree. With
// `--pch-diff`, the header's precompiled image gives the same errors
// (WP-1293).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:8 error "functions that differ only in their return type cannot be overloaded"
//@ diag: 16:15 error "redeclaration of 'le_var' with a different type: 'long' vs 'int'"
//@ diag: 20:23 error "redeclaration of 'le_global' with a different type: 'int' vs 'long'"
#include "Inputs/local_extern.h"

namespace le {
int user() {
  long le_fn(int);
  extern long le_var;
  return 0;
}
}
void g() { extern int le_global; }
