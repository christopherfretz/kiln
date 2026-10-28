// [dcl.typedef]/4: a typedef redeclared with a different type
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:14 error "typedef redefinition with different types ('long' vs 'int')"
//@ diag: 6:13 note "previous definition is here"
typedef int T;
typedef long T;
