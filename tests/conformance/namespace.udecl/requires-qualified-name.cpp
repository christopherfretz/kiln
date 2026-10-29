// [namespace.udecl]/1: a using-declarator names a nested-name-specifier
//@ expect: compile-fail
//@ strict-diags
//@ diag: 6:7 error "using declaration requires a qualified name"
int x;
using x;
