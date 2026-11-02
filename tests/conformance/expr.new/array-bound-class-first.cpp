// [expr.new]/8: a class-type first bound without a conversion function.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:20 error "converting 'S' to incompatible type '__size_t' (aka 'unsigned long')"
//@ requires: clang>=22
struct S {};
extern S s;
void f() { new int[s]; }
