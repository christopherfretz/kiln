// [expr.new]/2: a placeholder type needs exactly one initializer to deduce from.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 5:34 error "new expression for type 'auto' contains multiple constructor arguments"
void f() { auto *p = new auto(1, 2); }
