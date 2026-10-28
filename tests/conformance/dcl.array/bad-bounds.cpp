// [dcl.array]/1: the bound is a converted constant expression greater than zero
//@ expect: compile-fail
//@ strict-diags
//@ diag: 5:9 error "'neg' declared as an array with a negative size"
int neg[-1];
