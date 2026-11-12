// [temp.arg.explicit]/1, [expr.new]/2: the explicit template arguments of
// a header's call and a header's new-expression are checked at the
// instantiation, as without a precompiled header (WP-1320).
//@ expect: compile-fail
//@ diag: 9:3 error "no matching function for call to 'size'"
#include "Inputs/new_dtor_targs.h"

int f() {
  ndt::size<int>();
  return 0;
}
