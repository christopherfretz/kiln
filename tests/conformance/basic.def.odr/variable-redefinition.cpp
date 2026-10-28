// [basic.def.odr]/1: one definition per translation unit
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:5 error "redefinition of 'a'"
//@ diag: 6:5 note "previous definition is here"
int a = 1;
int a = 2;
