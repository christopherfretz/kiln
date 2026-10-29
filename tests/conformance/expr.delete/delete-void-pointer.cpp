// [expr.delete]/1: void* is not a pointer to an object type.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 5:19 error "cannot delete pointer to incomplete type 'void'"
void f(void *p) { delete p; }
