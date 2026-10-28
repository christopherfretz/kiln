// [dcl.ref]/5: a reference must be initialized
//@ expect: compile-fail
//@ strict-diags
//@ diag: 5:6 error "declaration of reference variable 'r' requires an initializer"
int &r;
