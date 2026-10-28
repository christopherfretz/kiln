// [dcl.init]/6: a block-scope extern declaration cannot have an initializer
//@ expect: compile-fail
//@ strict-diags
//@ diag: 5:23 error "declaration of block scope identifier with linkage cannot have an initializer"
void f() { extern int x = 1; }
