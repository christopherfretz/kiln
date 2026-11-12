// [dcl.init.general]/16, [temp.inst]/4, [over.match.ctor]: a dependent
// direct-initialization in a header's class template is checked at the
// instantiation, and the header's constructors are the candidates of a
// direct-initialization in the unit, as without a precompiled header
// (WP-1320).
//@ expect: compile-fail
//@ diag: 13:17 error "no matching constructor for initialization of 'gnp::pos<int>'"
#include "Inputs/global_new_paren.h"

int f() {
  gnp::pos<char *> c(nullptr);
  c = c + nullptr;
  gnp::pos<int> z(1, 2, 3);
  return 0;
}
