// [basic.def.odr]/1: a function defined twice
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:5 error "redefinition of 'f'"
//@ diag: 6:5 note "previous definition is here"
int f() { return 0; }
int f() { return 1; }
