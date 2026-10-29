// [expr.new]/2: a placeholder type needs exactly one initializer to deduce from.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 5:22 error "new expression for type 'auto' requires a constructor argument"
void f() { auto *p = new auto; }
