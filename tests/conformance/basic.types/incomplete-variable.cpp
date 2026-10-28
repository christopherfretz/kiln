// [basic.def]/5: a defined object has a complete type
//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:3 error "variable has incomplete type 'I'"
//@ diag: 7:8 note "forward declaration of 'I'"
//@ diag: 9:6 error "variable has incomplete type 'void'"
struct I;
I i;
void v;
