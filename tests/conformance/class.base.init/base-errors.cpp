// [class.base.init]/2, [class.access.base], [class.protected]: a base named
// both directly and as an inherited virtual base is ambiguous; a base or
// member constructor must be accessible from the derived class, where a
// protected base constructor (and destructor) is accessible but a protected
// constructor of a member subobject is not. [dcl.init.general]/16.9: a scalar
// member takes one expression.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:12 warning "direct base 'V' is inaccessible due to ambiguity:"
//@ diag: 24:25 error "base class initializer 'V' names both a direct base class and an inherited virtual base class"
//@ diag: 25:28 error "excess elements in scalar initializer"
//@ diag: 27:22 error "base class 'P' has private constructor"
//@ diag: 26:21 note "declared private here"
//@ diag: 28:23 error "field of type 'P' has private constructor"
//@ diag: 26:21 note "declared private here"
//@ diag: 28:14 error "field of type 'P' has protected destructor"
//@ diag: 26:58 note "declared protected here"
//@ diag: 32:23 error "field of type 'P' has protected default constructor"
//@ diag: 26:40 note "declared protected here"
//@ diag: 32:14 error "field of type 'P' has protected destructor"
//@ diag: 26:58 note "declared protected here"
struct V { V(int = 0); int v; };
struct A : virtual V {};
struct D : V, A { D() : V(1) {} };
struct H { int a, b; H() : a(1, 2) {} };
struct P { private: P(int); protected: P(); P(const P&); ~P(); };
struct Q : P { Q() : P(1) {} Q(int) : P() {} Q(const Q &q) : P(q) {} };
struct R { P p; R() : p(1) {} };
struct R2 : P {};
R2 r2;
R2 r3(r2);
struct W { P p; W() : p() {} };
