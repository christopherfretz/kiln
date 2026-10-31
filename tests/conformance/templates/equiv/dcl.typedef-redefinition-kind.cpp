//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:22 error "typedef redefinition with different types ('long' vs 'int')"
//@ diag: 8:13 note "previous definition is here"
//@ diag: 9:29 error "type alias redefinition with different types ('long' vs 'int')"
//@ diag: 9:7 note "previous definition is here"
// [dcl.typedef]/4: the diagnostic names the previous declaration's kind.
typedef int C; using C = long;
using D = int; typedef long D;
typedef int E; using E = int;
