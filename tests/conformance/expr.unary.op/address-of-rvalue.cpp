// [expr.unary.op]/3: & needs an lvalue
//@ expect: compile-fail
//@ strict-diags
//@ diag: 5:10 error "cannot take the address of an rvalue of type 'int'"
int *p = &5;
