// [dcl.init.general]/16.6: a scalar takes at most one parenthesized initializer.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 5:16 error "excess elements in scalar initializer"
void f() { new int(1, 2); }
