// [class.base.init]/9, [basic.life]/1, [expr.const]/5.8: a constructor
// that leaves out a base without a default constructor is diagnosed by
// Sema; clang's evaluator then runs it with the base subobject never
// constructed, and so does the Oven. Reading (`d.x`, through a `const B&`,
// a member of the base's own base), assigning or calling a member function
// of the base is "... object outside its lifetime", destroying it (a
// constexpr destructor of the base) is "destroying object ... whose
// lifetime has already ended", and a `constexpr` variable holding the
// object notes "constructor of base class 'B' is not called" at the
// base-specifier. The derived class's own members, a delegating
// constructor's target, and a constructor that does initialize the base
// are unaffected. Matches clang 19 and 22.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 47:13 error "constructor for 'D' must explicitly initialize the base class 'B' which does not have a default constructor"
//@ diag: 43:8 note "'B' declared here"
//@ diag: 54:15 error "constexpr variable 'v2' must be initialized by a constant expression"
//@ diag: 53:39 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 54:20 note "in call to 'f2()'"
//@ diag: 56:15 error "constexpr variable 'v3' must be initialized by a constant expression"
//@ diag: 55:36 note "member call on object outside its lifetime is not allowed in a constant expression"
//@ diag: 56:20 note "in call to 'f3()'"
//@ diag: 58:15 error "constexpr variable 'v4' must be initialized by a constant expression"
//@ diag: 57:50 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 58:20 note "in call to 'f4()'"
//@ diag: 60:15 error "constexpr variable 'v5' must be initialized by a constant expression"
//@ diag: 59:34 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 60:20 note "in call to 'f5()'"
//@ diag: 64:15 error "constexpr variable 'v7' must be initialized by a constant expression"
//@ diag: 63:31 note "assignment to object outside its lifetime is not allowed in a constant expression"
//@ diag: 64:20 note "in call to 'f7()'"
//@ diag: 65:13 error "constexpr variable 'd8' must be initialized by a constant expression"
//@ diag: 45:15 note "constructor of base class 'B' is not called"
//@ diag: 69:26 error "constructor for 'F' must explicitly initialize the base class 'E' which does not have a default constructor"
//@ diag: 68:8 note "'E' declared here"
//@ diag: 73:37 error "constructor for 'H' must explicitly initialize the base class 'G' which does not have a default constructor"
//@ diag: 72:8 note "'G' declared here"
//@ diag: 75:15 error "static assertion expression is not an integral constant expression"
//@ diag: 74:24 note "destroying object 'h' whose lifetime has already ended"
//@ diag: 74:24 note "in call to 'h.~H()'"
//@ diag: 75:15 note "in call to 'g2()'"
struct A { int a = 1; };
struct B : A { int x; constexpr B(int v) : x(v) {} constexpr int get() const { return x; } };
struct C { int c; constexpr C() : c(5) {} };
struct D : C, B {
  int y;
  constexpr D() : y(3) {}
  constexpr D(char) : D() {}
  constexpr D(int) : B(1), y(4) {}
};
constexpr int f1() { D d('a'); return d.y + d.c; }
static_assert(f1() == 8);
constexpr int f2() { D d('a'); return d.x; }
constexpr int v2 = f2();
constexpr int f3() { D d; return d.get(); }
constexpr int v3 = f3();
constexpr int f4() { D d; const B &b = d; return b.x; }
constexpr int v4 = f4();
constexpr int f5() { D d; return d.a; }
constexpr int v5 = f5();
constexpr int f6() { D d(0); return d.x + d.a; }
static_assert(f6() == 2);
constexpr int f7() { D d; d.x = 2; return d.x; }
constexpr int v7 = f7();
constexpr D d8('a');
constexpr D d9(0);
static_assert(d9.x == 1 && d9.y == 4);
struct E { int x; constexpr E(int v) : x(v) {} };
struct F : E { constexpr F() {} };
constexpr int g1() { F f; return 1; }
static_assert(g1() == 1);
struct G { int x; constexpr G(int v) : x(v) {} constexpr ~G() {} };
struct H : G { int y = 0; constexpr H() {} };
constexpr int g2() { H h; return 2; }
static_assert(g2() == 2);
