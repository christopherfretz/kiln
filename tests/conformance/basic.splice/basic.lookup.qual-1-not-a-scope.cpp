// [basic.lookup.qual.general]/1, [expr.prim.id.qual]/2 P2996R13: a
// splice-scope-specifier designates a namespace, a namespace alias, a
// class or an enumeration (a class or enum type alias included); a
// scalar type, a variable or a function is not a scope.
//@ expect: compile-fail
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 17:12 error "splice in a scope position does not designate a class, namespace or enumeration"
//@ diag: 18:12 error "splice in a scope position does not designate a class, namespace or enumeration"
//@ diag: 19:12 error "splice in a scope position does not designate a class, namespace or enumeration"
#include "kiln_meta.h"
namespace N { int x; }
void fn();
struct S { static int s; };
using SA = S;
int a = [: ^^int :]::x;
int b = [: ^^N::x :]::y;
int c = [: ^^fn :]::z;
int d = [: ^^SA :]::s;  // OK: an alias of a class
