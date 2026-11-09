// C++14 [except.spec]/14-15: an implicitly-declared special member's
// exception specification is the union of the type lists of the functions
// it directly invokes; an out-of-line definition or an overrider must match
// or be compatible with that computed list (clang's type-list form).  WP-1052.
//@ std: c++14
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 29:4 error "exception specification in declaration does not match previous declaration"
//@ diag: 28:24 note "previous declaration is here"
//@ diag: 31:4 warning "function previously declared with an implicit exception specification redeclared with an explicit exception specification"
//@ diag: 30:18 note "previous declaration is here"
//@ diag: 33:4 warning "function previously declared with an implicit exception specification redeclared with an explicit exception specification"
//@ diag: 32:12 note "previous declaration is here"
//@ diag: 35:4 error "exception specification in declaration does not match previous declaration"
//@ diag: 34:18 note "previous declaration is here"
//@ diag: 37:4 error "exception specification in declaration does not match previous declaration"
//@ diag: 36:18 note "previous declaration is here"
//@ diag: 39:8 error "exception specification of overriding function is more lax than base version"
//@ diag: 38:20 note "overridden virtual function is here"
//@ diag: 42:22 error "exception specification of overriding function is more lax than base version"
//@ diag: 38:20 note "overridden virtual function is here"
//@ diag: 46:4 error "exception specification in declaration does not match previous declaration"
//@ diag: 45:18 note "previous declaration is here"
struct E1 {}; struct E2 {};
struct M1 { ~M1() throw(E1); M1& operator=(const M1&) throw(E1); };
struct M2 { ~M2() throw(E2); };
struct A { M1 m; M2 n; ~A(); };
A::~A() throw(E1) {}
struct B { M1 m; ~B(); };
B::~B() throw(E1) {}
struct C { ~C(); };
C::~C() throw() {}
struct D { M1 m; ~D(); };
D::~D() throw() {}
struct F { M1 m; ~F(); };
F::~F() throw(E1, E2) {}
struct V { virtual ~V() throw(E1); virtual V& operator=(const V&) throw(); };
struct W : V { M2 n; };
struct X : V { M1 m; };
struct Y : V { M1 m; ~Y(); };
struct Z : V { M2 m; ~Z(); };
// constructors
struct CT { CT() throw(E1); CT(const CT&) throw(E2); };
struct G { CT c; G(); };
G::G() throw(E1) {}
struct H { CT c; H() = default; H(const H&) = default; };
H h; H h2(h);
int main(){}
