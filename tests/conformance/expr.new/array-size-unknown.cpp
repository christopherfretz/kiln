// [expr.new]/7: without an initializer, the array bound must be given.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 5:26 error "array size must be specified in new expression with no initializer"
int *f() { return new int[]; }
