// [basic.link]/11: redeclarations must have the same type
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:8 error "redefinition of 'b' with a different type: 'double' vs 'int'"
//@ diag: 6:5 note "previous definition is here"
int b;
double b;
