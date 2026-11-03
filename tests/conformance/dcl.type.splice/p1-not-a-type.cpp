// [dcl.type.splice]/1 P2996R13: a splice-type-specifier designates a type
// (or a type alias); a namespace, a variable or a function in a type
// position is ill-formed, with or without `typename`.
//@ expect: compile-fail
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 15:14 error "splice in a type position does not designate a type"
//@ diag: 16:14 error "splice in a type position does not designate a type"
//@ diag: 17:13 error "splice in a type position does not designate a type"
#include "kiln_meta.h"
namespace N {}
int v;
void fn();
using A = [: ^^N :];
using B = [: ^^v :];
typename [: ^^fn :] *p;
