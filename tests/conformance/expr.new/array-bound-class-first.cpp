// [expr.new]/8: a class-type first bound without a conversion function.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:20 error "converting 'S' to incompatible type 'unsigned long'"
struct S {};
extern S s;
void f() { new int[s]; }
