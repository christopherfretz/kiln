// [dcl.type.splice]/2, [basic.splice]/2 P2996R13: the operand of a type
// splice is a constant expression of type std::meta::info that reflects
// a type; anything else is ill-formed.
//@ expect: compile-fail
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 19:6 error "'rv' is initialized with a consteval-only value but is not declared 'constexpr'"
//@ diag: 20:13 error "splice operand must be a constant expression"
//@ diag: 20:13 note "read of non-constexpr variable 'rv' is not allowed in a constant expression"
//@ diag: 19:6 note "declared here"
//@ diag: 21:13 error "splice in a type position does not designate a type"
//@ diag: 22:13 error "splice operand of type 'int' is not a reflection"
//@ diag: 23:13 error "splice in a type position does not designate a type"
//@ diag: 24:13 error "splice in a type position does not designate a type"
#include "kiln_meta.h"
using namespace std::meta;
int n = 0;
info rv = ^^int;
typename [: rv :] a;      // not a constant
typename [: ^^n :] b;     // a variable, not a type
typename [: 42 :] c;      // not a reflection
typename [: info{} :] d;  // the null reflection
typename [: ^^std :] e;   // a namespace
