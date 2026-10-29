// [expr.new]/1: a reference type is not an object type.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 5:17 error "cannot allocate reference type 'int &' with new"
void f() { new (int &); }
