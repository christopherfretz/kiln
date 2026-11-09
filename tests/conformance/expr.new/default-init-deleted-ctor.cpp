// [expr.new]/23, [dcl.init.general]/7: a new-expression that default-
// initializes a class object calls the default constructor that overload
// resolution selects, so a deleted or implicitly deleted one is an error
// even though it would be "trivial" (WP-1109; kiln used to skip the
// selection when the class's trivial-default-constructor bit was set).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:16 error "call to deleted constructor of 'D'"
//@ diag: 19:12 note "'D' has been explicitly marked deleted here"
//@ diag: 21:23 error "call to implicitly-deleted default constructor of 'R'"
//@ diag: 19:45 note "field 'r' of reference type 'int &' would not be initialized"
//@ diag: 21:30 error "call to implicitly-deleted default constructor of 'C'"
//@ diag: 19:72 note "field 'c' of const-qualified type 'const int' would not be initialized"
//@ diag: 21:37 error "call to deleted constructor of 'D[2]'"
//@ diag: 19:12 note "'D' has been explicitly marked deleted here"
//@ diag: 21:47 error "call to implicitly-deleted default constructor of 'X'"
//@ diag: 20:14 note "field 'd' has a deleted default constructor"
//@ diag: 19:12 note "'D' has been explicitly marked deleted here"
struct D { D() = delete; }; struct R { int& r; }; struct C { const int c; };
struct X { D d; };
void f() { new D; new R; new C; new D[2]; new X; }
