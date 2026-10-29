// [dcl.spec.auto.general]/13: a redeclaration must use the same placeholder
// (or the same deduced type).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:5 error "functions that differ only in their return type cannot be overloaded"
//@ diag: 9:6 note "previous declaration is here"
//@ diag: 12:6 error "functions that differ only in their return type cannot be overloaded"
//@ diag: 11:6 note "previous definition is here"
auto f();
int f();
auto g() { return 1; }
long g();
