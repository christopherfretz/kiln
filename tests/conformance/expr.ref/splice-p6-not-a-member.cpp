// [expr.ref]/6 P2996R13: in `E1.[: r :]`, r shall designate a member of
// E1's class T1 or of a base of T1 (or a direct base relationship of T1):
// a namespace-scope variable, a member of an unrelated class, a member of
// a class derived from T1 and a type are not.
//@ expect: compile-fail
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 19:16 error "splice in a member access position does not designate a member of 'S'"
//@ diag: 20:16 error "splice in a member access position does not designate a member of 'S'"
//@ diag: 21:17 error "splice in a member access position does not designate a member of 'S'"
//@ diag: 22:16 error "splice in a member access position does not designate a member of 'S'"
#include "kiln_meta.h"
namespace N { int x; }
struct S { int m; };
struct O { int m; };
struct D : S { int d; };
int f(S& s, S* p) {
  int a = s.[: ^^N::x :];
  int b = s.[: ^^O::m :];
  int c = p->[: ^^D::d :];
  int e = s.[: ^^int :];
  return a + b + c + e + s.[: ^^S::m :];
}
