// [basic.life]/9, [class.dtor]/13, [expr.const]: destroying a derived object
// whose trivially destructible base its constructor never constructed ends the
// lifetime of an object outside it ("destroying object 'd' whose lifetime has
// already ended", in the destructor's frame); a trivial copy of an object with
// such a base (directly or in a member) is "constructor of base class 'B' is not
// called" at the base-specifier (clang's CheckFullyInitialized). WP-409.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 48:35 error "constructor for 'D1' must explicitly initialize the base class 'Em' which does not have a default constructor"
//@ diag: 47:8 note "'Em' declared here"
//@ diag: 50:15 error "static assertion expression is not an integral constant expression"
//@ diag: 49:25 note "destroying object 'd' whose lifetime has already ended"
//@ diag: 49:25 note "in call to 'd.~D1()'"
//@ diag: 50:15 note "in call to 'f1()'"
//@ diag: 51:32 error "constructor for 'D2' must explicitly initialize the base class 'B' which does not have a default constructor"
//@ diag: 45:8 note "'B' declared here"
//@ diag: 53:15 error "static assertion expression is not an integral constant expression"
//@ diag: 52:25 note "destroying object 'd' whose lifetime has already ended"
//@ diag: 52:25 note "in call to 'd.~D2()'"
//@ diag: 53:15 note "in call to 'f2()'"
//@ diag: 55:15 error "static assertion expression is not an integral constant expression"
//@ diag: 54:38 note "destroying object '{*new D2#0}' whose lifetime has already ended"
//@ diag: 54:38 note "in call to '{*new D2#0}.~D2()'"
//@ diag: 55:15 note "in call to 'f3()'"
//@ diag: 57:30 error "constructor for 'D3' must explicitly initialize the base class 'B' which does not have a default constructor"
//@ diag: 45:8 note "'B' declared here"
//@ diag: 59:15 error "static assertion expression is not an integral constant expression"
//@ diag: 58:25 note "destroying object 'd' whose lifetime has already ended"
//@ diag: 58:25 note "in call to 'd.~D3()'"
//@ diag: 59:15 note "in call to 'f4()'"
//@ diag: 60:37 error "constructor for 'E' must explicitly initialize the base class 'B' which does not have a default constructor"
//@ diag: 45:8 note "'B' declared here"
//@ diag: 62:15 error "static assertion expression is not an integral constant expression"
//@ diag: 60:12 note "constructor of base class 'B' is not called"
//@ diag: 61:34 note "in call to 'E(e)'"
//@ diag: 62:15 note "in call to 'f5()'"
//@ diag: 65:15 error "static assertion expression is not an integral constant expression"
//@ diag: 60:12 note "constructor of base class 'B' is not called"
//@ diag: 64:34 note "in call to 'W(w)'"
//@ diag: 65:15 note "in call to 'f6()'"
//@ diag: 67:15 error "static assertion expression is not an integral constant expression"
//@ diag: 60:12 note "constructor of base class 'B' is not called"
//@ diag: 66:29 note "in call to 'E(e)'"
//@ diag: 67:15 note "in call to 'f7()'"
struct B { int x; constexpr B(int) : x(1) {} };
struct M { constexpr ~M() {} };
struct Em { constexpr Em(int) {} };
struct D1 : Em { int z; constexpr D1() {} constexpr ~D1() {} };
constexpr int f1() { D1 d; return 0; }
static_assert(f1() == 0);
struct D2 : B { M m; constexpr D2() {} };
constexpr int f2() { D2 d; return 0; }
static_assert(f2() == 0);
constexpr int f3() { D2 *p = new D2; delete p; return 0; }
static_assert(f3() == 0);
struct A { int a = 1; };
struct D3 : A, B { constexpr D3() {} constexpr ~D3() {} };
constexpr int f4() { D3 d; return 0; }
static_assert(f4() == 0);
struct E : B { int y = 2; constexpr E() {} };
constexpr int f5() { E e; E e2 = e; e2 = e; return 0; }
static_assert(f5() == 0);
struct W { E e; };
constexpr int f6() { W w; W w2 = w; return 0; }
static_assert(f6() == 0);
constexpr int f7() { E e; E e2(static_cast<E&&>(e)); return 0; }
static_assert(f7() == 0);
