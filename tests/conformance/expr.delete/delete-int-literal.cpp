// [expr.delete]/1: a null pointer constant of integer type is not a pointer.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 5:12 error "cannot delete expression of type 'int'"
void f() { delete 0; }
