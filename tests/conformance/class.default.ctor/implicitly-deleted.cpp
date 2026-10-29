// [class.default.ctor]/2: a defaulted default constructor is deleted for a
// reference member without a default member initializer, a const member that
// is not const-default-constructible, a variant member with a non-trivial
// default constructor, an all-const union, and a subobject whose default
// constructor is deleted or inaccessible; [dcl.init.general]/7 uses it.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 30:3 error "call to implicitly-deleted default constructor of 'R'"
//@ diag: 29:17 note "default constructor of 'R' is implicitly deleted because field 'r' of reference type 'int &' would not be initialized"
//@ diag: 32:3 error "call to implicitly-deleted default constructor of 'C'"
//@ diag: 31:22 note "default constructor of 'C' is implicitly deleted because field 'c' of const-qualified type 'const int' would not be initialized"
//@ diag: 33:39 error "binding reference of type 'int' to value of type 'const int' drops 'const' qualifier"
//@ diag: 37:3 error "call to implicitly-deleted default constructor of 'H'"
//@ diag: 36:15 note "default constructor of 'H' is implicitly deleted because field 'n' has a deleted default constructor"
//@ diag: 35:13 note "'NC' has been explicitly marked deleted here"
//@ diag: 39:3 error "call to implicitly-deleted default constructor of 'D'"
//@ diag: 38:12 note "default constructor of 'D' is implicitly deleted because base class 'NC' has a deleted default constructor"
//@ diag: 35:13 note "'NC' has been explicitly marked deleted here"
//@ diag: 42:4 error "call to implicitly-deleted default constructor of 'UU'"
//@ diag: 41:14 note "default constructor of 'UU' is implicitly deleted because variant field 'u' has a non-trivial default constructor"
//@ diag: 47:3 error "call to implicitly-deleted default constructor of 'B'"
//@ diag: 46:14 note "default constructor of 'B' is implicitly deleted because field 'a' has multiple default constructors"
//@ diag: 50:3 error "call to implicitly-deleted default constructor of 'Q'"
//@ diag: 49:12 note "default constructor of 'Q' is implicitly deleted because base class 'P' has an inaccessible default constructor"
//@ diag: 52:3 error "call to implicitly-deleted default constructor of 'V'"
//@ diag: 51:7 note "default constructor of 'V' is implicitly deleted because all data members are const-qualified"
//@ diag: 54:3 error "call to implicitly-deleted default constructor of 'W'"
//@ diag: 53:12 note "default constructor of 'W' is implicitly deleted because all data members of an anonymous union member are const-qualified"
struct R { int &r; };
R r;
struct C { const int c; };
C c;
struct CI { const int c = 1; int &r = c2; static const int c2 = 0; };
CI ci;
struct NC { NC() = delete; };
struct H { NC n; };
H h;
struct D : NC { };
D d;
struct U { U(); };
union UU { U u; int i; };
UU uu;
union UI { U u; int i = 0; };
UI ui;
struct A { A(); A(int = 0); };
struct B { A a; };
B b;
struct P { private: P(); };
struct Q : P { };
Q q;
union V { const int a; const int b; };
V v;
struct W { union { const int a; const float b; }; };
W w;


