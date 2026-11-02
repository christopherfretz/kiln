// [class.base.init]/9, [basic.life]/1, [expr.const]/5.8: an EMPTY base
// that a constructor leaves out (Sema diagnosed it: no default
// constructor) is never constructed either. Calling a member function of
// it is "member call on object outside its lifetime"; a `constexpr`
// variable holding the object notes "constructor of base class 'B' is
// not called" at the first such base-specifier as written; destroying it
// (a constexpr destructor of the base) is "destroying object ..." naming
// the derived object. An empty base the constructor does initialize is
// unaffected. Matches clang 19 and 22.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 51:37 error "constructor for 'D' must explicitly initialize the base class 'B' which does not have a default constructor"
//@ diag: 50:8 note "'B' declared here"
//@ diag: 53:15 error "constexpr variable 'v1' must be initialized by a constant expression"
//@ diag: 52:36 note "member call on object outside its lifetime is not allowed in a constant expression"
//@ diag: 53:20 note "in call to 'f1()'"
//@ diag: 55:15 error "constexpr variable 'v2' must be initialized by a constant expression"
//@ diag: 54:52 note "member call on object outside its lifetime is not allowed in a constant expression"
//@ diag: 55:20 note "in call to 'f2()'"
//@ diag: 56:13 error "constexpr variable 'd3' must be initialized by a constant expression"
//@ diag: 51:12 note "constructor of base class 'B' is not called"
//@ diag: 57:27 error "constructor for 'D4' must explicitly initialize the base class 'B' which does not have a default constructor"
//@ diag: 50:8 note "'B' declared here"
//@ diag: 59:15 error "constexpr variable 'v4' must be initialized by a constant expression"
//@ diag: 58:37 note "member call on object outside its lifetime is not allowed in a constant expression"
//@ diag: 59:20 note "in call to 'f4()'"
//@ diag: 64:38 error "constructor for 'D6' must explicitly initialize the base class 'E' which does not have a default constructor"
//@ diag: 63:8 note "'E' declared here"
//@ diag: 66:15 error "static assertion expression is not an integral constant expression"
//@ diag: 65:25 note "destroying object 'd' whose lifetime has already ended"
//@ diag: 65:25 note "in call to 'd.~D6()'"
//@ diag: 66:15 note "in call to 'f6()'"
//@ diag: 68:15 error "static assertion expression is not an integral constant expression"
//@ diag: 67:38 note "destroying object '{*new D6#0}' whose lifetime has already ended"
//@ diag: 67:38 note "in call to '{*new D6#0}.~D6()'"
//@ diag: 68:15 note "in call to 'f7()'"
//@ diag: 71:15 error "static assertion expression is not an integral constant expression"
//@ diag: 70:36 note "destroying object '{*new W#0}.m' whose lifetime has already ended"
//@ diag: 70:36 note "in call to '{*new W#0}.m.~D6()'"
//@ diag: 70:36 note "in call to '{*new W#0}.~W()'"
//@ diag: 71:15 note "in call to 'f8()'"
//@ diag: 73:15 error "constexpr variable 'v9' must be initialized by a constant expression"
//@ diag: 72:29 note "destroying object 'D6()' whose lifetime has already ended"
//@ diag: 72:29 note "in call to 'D6().~D6()'"
//@ diag: 73:20 note "in call to 'f9()'"
//@ diag: 75:40 error "constructor for 'M' must explicitly initialize the base class 'B' which does not have a default constructor"
//@ diag: 50:8 note "'B' declared here"
//@ diag: 76:13 error "constexpr variable 'm10' must be initialized by a constant expression"
//@ diag: 75:15 note "constructor of base class 'B' is not called"
struct B { constexpr B(int) {} constexpr int get() const { return 7; } };
struct D : B { int y = 1; constexpr D() {} };
constexpr int f1() { D d; return d.get(); }
constexpr int v1 = f1();
constexpr int f2() { D d; const B &b = d; return b.get(); }
constexpr int v2 = f2();
constexpr D d3;
struct D4 : B { constexpr D4() {} };
constexpr int f4() { D4 d; return d.get(); }
constexpr int v4 = f4();
struct D5 : B { constexpr D5() : B(1) {} };
constexpr int f5() { D5 d; return d.get(); }
static_assert(f5() == 7);
struct E { constexpr E(int) {} constexpr ~E() {} };
struct D6 : E { int y = 0; constexpr D6() {} };
constexpr int f6() { D6 d; return 1; }
static_assert(f6() == 1);
constexpr int f7() { D6 *p = new D6; delete p; return 1; }
static_assert(f7() == 1);
struct W { int a = 0; D6 m; };
constexpr int f8() { W *p = new W; delete p; return 1; }
static_assert(f8() == 1);
constexpr int f9() { return D6().y; }
constexpr int v9 = f9();
struct C { int x; constexpr C(int v) : x(v) {} };
struct M : C, B { int z = 0; constexpr M() : C(1) {} };
constexpr M m10;
