// [class.base.init]/9, [class.default.ctor]/2: a user-written constructor that
// leaves a reference or const member, a base or member with no default
// constructor, or (in the most derived class) a virtual base without an
// initializer is ill-formed; the implicit default constructor of such a class
// is instead defined as deleted, and using it is an error.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 28:20 error "constructor for 'I' must explicitly initialize the reference member 'r'"
//@ diag: 28:17 note "declared here"
//@ diag: 29:25 error "constructor for 'C' must explicitly initialize the const member 'c'"
//@ diag: 29:22 note "declared here"
//@ diag: 31:16 error "constructor for 'D' must explicitly initialize the base class 'B' which does not have a default constructor"
//@ diag: 30:8 note "'B' declared here"
//@ diag: 32:19 error "constructor for 'UU' must explicitly initialize the member 'u' which does not have a default constructor"
//@ diag: 32:16 note "member is declared here"
//@ diag: 27:8 note "'ND' declared here"
//@ diag: 34:16 error "constructor for 'W' must explicitly initialize the base class 'B' which does not have a default constructor"
//@ diag: 30:8 note "'B' declared here"
//@ diag: 36:4 error "call to implicitly-deleted default constructor of 'IR'"
//@ diag: 35:18 note "default constructor of 'IR' is implicitly deleted because field 'r' of reference type 'int &' would not be initialized"
//@ diag: 38:4 error "call to implicitly-deleted default constructor of 'IC'"
//@ diag: 37:23 note "default constructor of 'IC' is implicitly deleted because field 'c' of const-qualified type 'const int' would not be initialized"
//@ diag: 40:4 error "call to implicitly-deleted default constructor of 'IB'"
//@ diag: 39:13 note "default constructor of 'IB' is implicitly deleted because base class 'B' has no default constructor"
//@ diag: 42:4 error "call to implicitly-deleted default constructor of 'IM'"
//@ diag: 41:16 note "default constructor of 'IM' is implicitly deleted because field 'm' has no default constructor"
struct ND { ND(int); };
struct I { int &r; I() {} };
struct C { const int c; C() {} };
struct B { B(int); };
struct D : B { D() {} };
struct UU { ND u; UU() {} };
struct V : virtual B { V() : B(1) {} };
struct W : V { W() {} };
struct IR { int &r; };
IR ir;
struct IC { const int c; };
IC ic;
struct IB : B {};
IB ib;
struct IM { ND m; };
IM im;
