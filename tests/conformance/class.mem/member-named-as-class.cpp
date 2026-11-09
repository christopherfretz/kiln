// [class.mem.general]/22: a static data member, a member type, a member
// template, an enumerator of an unscoped member enumeration and a member
// named by a using-declaration have a name different from the class; so
// does a non-static data member of a class with a user-declared
// constructor. WP-1015 (clang's DiagnoseClassNameShadow).
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:24 error "member 'S0' has the same name as its class"
//@ diag: 26:18 error "member 'S1' has the same name as its class"
//@ diag: 27:20 error "member 'S2' has the same name as its class"
//@ diag: 28:25 error "member 'S3' has the same name as its class"
//@ diag: 29:19 error "member 'S4' has the same name as its class"
//@ diag: 30:41 error "member 'S5' has the same name as its class"
//@ diag: 31:45 error "member 'S6' has the same name as its class"
//@ diag: 32:40 error "member 'S7' has the same name as its class"
//@ diag: 33:22 error "member 'S8' has the same name as its class"
//@ diag: 35:26 error "member 'D0' has the same name as its class"
//@ diag: 36:26 error "member 'D1' has the same name as its class"
//@ diag: 37:26 error "member 'D2' has the same name as its class"
//@ diag: 38:16 error "member 'F' has the same name as its class"
//@ diag: 39:24 error "member of anonymous union redeclares 'U'"
//@ diag: 39:8 note "previous declaration is here"
//@ diag: 40:42 error "member 'T0' has the same name as its class"
struct S0 { static int S0; };
struct S1 { enum S1 { }; };
struct S2 { struct S2; };
struct S3 { typedef int S3; };
struct S4 { using S4 = int; };
struct S5 { template<typename T> struct S5; };
struct S6 { template<typename T> static int S6; };
struct S7 { template<typename T> using S7 = int; };
struct S8 { enum E { S8 }; };
struct B { static int D0; enum D1 {}; union { int D2; }; };
struct D0 : B { using B::D0; };
struct D1 : B { using B::D1; };
struct D2 : B { using B::D2; };
struct F { int F; F(); };
struct U { union { int U; }; };
template<class T> struct T0 { static int T0; };
