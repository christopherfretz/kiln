// [dcl.fct]/13, [basic.link]/11, [over.dcl]: through a precompiled header,
// a function declared through a typedef keeps its type: a redeclaration
// differing only in the return type is an error, and a noexcept typedef
// keeps the function non-throwing (WP-1369).
//@ expect: compile-fail
//@ diag: 9:10 error "functions that differ only in their return type cannot be overloaded"
//@ diag: 10:15 error "static assertion failed"
#include "Inputs/typedef_fns.h"
long tf::twice(int v);
static_assert(!noexcept(tf::thrice(1)));
