// [stmt.return]/2, [dcl.init.general]/16: the operand of a `return` in a
// non-void function copy-initializes the result; a `void` operand cannot,
// and is diagnosed as that conversion at the operand, as in clang.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:18 error "cannot initialize return object of type 'int' with an rvalue of type 'void'"
//@ diag: 10:22 error "cannot initialize return object of type 'long' with an rvalue of type 'void'"
void g();
int b() { return g(); }
long c(int) { return (void)0; }
void ok() { return g(); }
