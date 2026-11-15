// A constructor declarator with a parameter of an unknown type is invalid:
// no "constructor cannot be redeclared" follows it, and a class made invalid
// by such a member gets no "must explicitly initialize the reference member"
// (clang). A valid class is still diagnosed (V). Reduced from real code.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:22 error "unknown type name 'a'"
//@ diag: 15:35 error "unknown type name 'b'"
//@ diag: 15:42 error "unknown type name 'c'"
//@ diag: 16:20 error "unknown type name 'zz'"
//@ diag: 17:25 error "unknown type name 'yy'"
//@ diag: 19:20 error "constructor for 'V' must explicitly initialize the reference member 'r'"
//@ diag: 19:17 note "declared here"
struct U { U(int); U(a); };
struct U2 { U2(int, int); U2(int, b); U2(c); };
struct A { int &r; zz x; A() {} };
struct B { const int k; yy y; B(); };
B::B() {}
struct V { int &r; V() {} };  // a valid class is still diagnosed
