// [class.base.init]/9, [basic.life]/1, [expr.const]/5.8: destroying an
// object whose base a constructor never constructed is "destroying object
// ... whose lifetime has already ended", noted where the destruction
// starts (clang's `HandleDestructionImpl` passes its call range down to
// the subobjects): the `delete` or `delete[]` expression, the variable,
// the temporary; never the implicit destructor of the class. Matches
// clang 19 and 22.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 34:37 error "constructor for 'D' must explicitly initialize the base class 'B' which does not have a default constructor"
//@ diag: 33:8 note "'B' declared here"
//@ diag: 36:15 error "static assertion expression is not an integral constant expression"
//@ diag: 35:36 note "destroying object '{*new D#0}' whose lifetime has already ended"
//@ diag: 35:36 note "in call to '{*new D#0}.~D()'"
//@ diag: 36:15 note "in call to 'f1()'"
//@ diag: 39:15 error "static assertion expression is not an integral constant expression"
//@ diag: 38:36 note "destroying object '{*new W#0}.m' whose lifetime has already ended"
//@ diag: 38:36 note "in call to '{*new W#0}.m.~D()'"
//@ diag: 38:36 note "in call to '{*new W#0}.~W()'"
//@ diag: 39:15 note "in call to 'f2()'"
//@ diag: 41:15 error "static assertion expression is not an integral constant expression"
//@ diag: 40:39 note "destroying object '{*new D[2]#0}[1]' whose lifetime has already ended"
//@ diag: 40:39 note "in call to '{*new D[2]#0}[1].~D()'"
//@ diag: 41:15 note "in call to 'f3()'"
//@ diag: 43:15 error "static assertion expression is not an integral constant expression"
//@ diag: 42:24 note "destroying object 'd' whose lifetime has already ended"
//@ diag: 42:24 note "in call to 'd.~D()'"
//@ diag: 43:15 note "in call to 'f4()'"
//@ diag: 45:15 error "constexpr variable 'v5' must be initialized by a constant expression"
//@ diag: 44:29 note "destroying object 'D()' whose lifetime has already ended"
//@ diag: 44:29 note "in call to 'D().~D()'"
//@ diag: 45:20 note "in call to 'f5()'"
struct B { int x; constexpr B(int v) : x(v) {} constexpr ~B() {} };
struct D : B { int y = 0; constexpr D() {} };
constexpr int f1() { D *p = new D; delete p; return 1; }
static_assert(f1() == 1);
struct W { int a = 0; D m; };
constexpr int f2() { W *p = new W; delete p; return 1; }
static_assert(f2() == 1);
constexpr int f3() { D *p = new D[2]; delete[] p; return 1; }
static_assert(f3() == 1);
constexpr int f4() { D d; return 1; }
static_assert(f4() == 1);
constexpr int f5() { return D().y; }
constexpr int v5 = f5();
