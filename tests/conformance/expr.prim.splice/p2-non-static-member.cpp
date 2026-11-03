// [expr.prim.splice]/2, [expr.prim.id.general]/4 P2996R13: a splice
// designating a non-static data member is not an implicit member access,
// so outside `&` and an unevaluated operand it is ill-formed, even inside
// a member function of the class.
//@ expect: compile-fail
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 14:22 error "invalid use of non-static data member 'm'"
//@ diag: 16:9 error "invalid use of non-static data member 'm'"
#include "kiln_meta.h"
struct S {
  int m;
  int get() { return [: ^^S::m :]; }
};
int x = [: ^^S::m :];
static_assert(sizeof([: ^^S::m :]) == sizeof(int));  // unevaluated: OK
