// [expr.new]/9 + [dcl.init.aggr]: more initializers than elements.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 5:29 error "excess elements in array initializer"
void f() { new int[2]{1, 2, 3}; }
