// [expr.spaceship]/4-5: narrowing of the converted operands of a builtin
// `<=>` is ill-formed; an ordered comparison of a pointer and zero.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:10 error "argument to 'operator<=>' evaluates to -1, which cannot be narrowed to type 'unsigned int'"
//@ diag: 11:10 error "argument to 'operator<=>' cannot be narrowed from type 'int' to 'unsigned int'"
//@ diag: 12:13 error "ordered comparison between pointer and zero ('int *' and 'int')"
#include "Inputs/compare.h"
int i;
auto a = -1 <=> 1u;
auto b = i <=> 1u;
auto c = &i <=> 0;
long l;
auto d = l <=> 1.0f;
