// [class.base.init]/9: a constructor that does not initialize a base or
// member lacking a default constructor is ill-formed; each such base and
// member is reported, not only the first.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 39:19 error "constructor for 'C' must explicitly initialize the base class 'A' which does not have a default constructor"
//@ diag: 37:8 note "'A' declared here"
//@ diag: 39:19 error "constructor for 'C' must explicitly initialize the base class 'B' which does not have a default constructor"
//@ diag: 38:8 note "'B' declared here"
//@ diag: 40:36 error "constructor for 'D' must explicitly initialize the base class 'A' which does not have a default constructor"
//@ diag: 37:8 note "'A' declared here"
//@ diag: 40:36 error "constructor for 'D' must explicitly initialize the base class 'B' which does not have a default constructor"
//@ diag: 38:8 note "'B' declared here"
//@ diag: 40:36 error "constructor for 'D' must explicitly initialize the member 'a' which does not have a default constructor"
//@ diag: 40:28 note "member is declared here"
//@ diag: 37:8 note "'A' declared here"
//@ diag: 40:36 error "constructor for 'D' must explicitly initialize the member 'b' which does not have a default constructor"
//@ diag: 40:33 note "member is declared here"
//@ diag: 38:8 note "'B' declared here"
//@ diag: 41:19 error "constructor for 'E' must explicitly initialize the base class 'B' which does not have a default constructor"
//@ diag: 38:8 note "'B' declared here"
//@ diag: 42:27 error "constructor for 'F' must explicitly initialize the base class 'A' which does not have a default constructor"
//@ diag: 37:8 note "'A' declared here"
//@ diag: 42:27 error "constructor for 'F' must explicitly initialize the base class 'B' which does not have a default constructor"
//@ diag: 38:8 note "'B' declared here"
//@ diag: 43:22 error "constructor for 'G' must explicitly initialize the member 'a' which does not have a default constructor"
//@ diag: 43:14 note "member is declared here"
//@ diag: 37:8 note "'A' declared here"
//@ diag: 43:22 error "constructor for 'G' must explicitly initialize the member 'b' which does not have a default constructor"
//@ diag: 43:19 note "member is declared here"
//@ diag: 38:8 note "'B' declared here"
//@ diag: 44:16 error "constructor for 'H' must explicitly initialize the base class 'A' which does not have a default constructor"
//@ diag: 37:8 note "'A' declared here"
//@ diag: 44:16 error "constructor for 'H' must explicitly initialize the member 'b' which does not have a default constructor"
//@ diag: 44:42 note "member is declared here"
//@ diag: 38:8 note "'B' declared here"
struct A { A(int); };
struct B { B(int); };
struct C : A, B { C() {} };
struct D : A, B { int x; A a; B b; D() {} };
struct E : A, B { E() : A(1) {} };
struct F : virtual A, B { F() {} };
struct G { A a; B b; G() {} };
struct H : A { H(int) : x(1) {} int x; B b; };
