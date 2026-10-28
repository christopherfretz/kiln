// [expr.prim.id.unqual]/1: a name must be declared before use
//@ expect: compile-fail
//@ strict-diags
//@ diag: 5:18 error "use of undeclared identifier 'undeclared_x'"
int f() { return undeclared_x; }
