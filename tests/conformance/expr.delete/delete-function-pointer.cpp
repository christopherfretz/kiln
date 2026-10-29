// [expr.delete]/1: a pointer to function cannot be deleted.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 6:12 error "cannot delete expression of type 'void ()'"
void g();
void f() { delete g; }
