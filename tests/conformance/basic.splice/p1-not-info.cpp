// [basic.splice]/1 P2996R13: the operand of a splice-specifier is a
// converted constant expression of type std::meta::info; in a type, an
// expression and a scope position alike.
//@ expect: compile-fail
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 13:14 error "splice operand of type 'const int' is not a reflection"
//@ diag: 14:12 error "splice operand of type 'int' is not a reflection"
//@ diag: 15:12 error "splice operand of type 'int' is not a reflection"
#include "kiln_meta.h"
constexpr int n = 3;
using T = [: n :];
int a = [: 42 :];
int b = [: n + 1 :]::x;
