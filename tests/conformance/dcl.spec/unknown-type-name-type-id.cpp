// [dcl.type.general]/1, [dcl.name]/1: an undeclared identifier where only
// a type-specifier can stand, the first token of a type-id (a
// new-type-id, a cast's or alias-declaration's type-id, a trailing
// return type, a default template argument, an exception-declaration,
// a for-range or condition declaration, a lambda parameter), is
// "unknown type name", as clang's `ParseImplicitInt` reports it; the
// recovered type is invalid, so the cast or expression using it says
// nothing more (WP-186).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 29:26 error "unknown type name 'Zork'"
//@ diag: 30:26 error "unknown type name 'Zork'"
//@ diag: 31:26 error "unknown type name 'Zork'"
//@ diag: 32:37 error "unknown type name 'Zork'"
//@ diag: 33:42 error "unknown type name 'Zork'"
//@ diag: 34:36 error "unknown type name 'Zork'"
//@ diag: 35:12 error "unknown type name 'Zork'"
//@ diag: 36:12 error "unknown type name 'Zork'"
//@ diag: 37:9 error "unknown type name 'Zork'"
//@ diag: 38:27 error "unknown type name 'Zork'"
//@ diag: 39:14 error "unknown type name 'Zork'"
//@ diag: 40:18 error "unknown type name 'Zork'"
//@ diag: 41:21 error "unknown type name 'Zork'"
//@ diag: 42:17 error "use of undeclared identifier 'Zork'"
//@ diag: 42:23 error "use of undeclared identifier 'p'"
//@ diag: 43:22 error "unknown type name 'Zork'"
//@ diag: 44:20 error "use of undeclared identifier 'Zork'"
//@ diag: 44:26 error "expected expression"
void a1() { auto p = new Zork; }
void a2() { auto p = new Zork(1); }
void a3() { auto p = new Zork[3]; }
void a4(int *q) { (void)static_cast<Zork *>(q); }
void a5(int *q) { (void)reinterpret_cast<Zork>(q); }
void a6(int *q) { (void)const_cast<Zork *>(q); }
using X1 = Zork;
using X2 = Zork *;
typedef Zork X3;
void b3() { try {} catch (Zork &) {} }
auto b4() -> Zork;
void b5() { for (Zork z : {1}) {} }
template <class T = Zork> struct D {};
void b7() { if (Zork *p = 0) {} }
void b8() { (void)[](Zork z) {}; }
void b9() { (void)(Zork *)0; }
