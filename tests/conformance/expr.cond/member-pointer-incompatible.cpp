// [expr.cond]/7.3, [expr.type]/4.6: pointers to members of unrelated
// classes have no composite pointer type.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:14 error "incompatible operand types ('int A::*' and 'int B::*')"
struct A { int m; };
struct B { int m; };
bool b;
void f() { b ? &A::m : &B::m; }
