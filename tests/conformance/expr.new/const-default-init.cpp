// [dcl.init.general]/8: a const object must be initialized.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 5:22 error "default initialization of an object of const type 'const int'"
void f() { new const int; }
