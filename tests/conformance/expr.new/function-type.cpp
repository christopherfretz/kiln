// [expr.new]/1: a function type is not an object type.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 6:16 error "cannot allocate function type 'F' (aka 'int (int)') with new"
using F = int(int);
void f() { new F; }
