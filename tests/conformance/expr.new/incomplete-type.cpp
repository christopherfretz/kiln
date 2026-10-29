// [expr.new]/1: the allocated type must be a complete object type.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:16 error "allocation of incomplete type 'S'"
//@ diag: 6:8 note "forward declaration of 'S'"
struct S;
void f() { new S; }
