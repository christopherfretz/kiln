// [dcl.constexpr]/6, [expr.const]/7: a constexpr variable must have
// constant destruction: its destruction, evaluated at the end of the
// initialization, must be a constant expression. The destructor runs with
// the object no longer const (it may modify the members).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:13 error "constexpr variable 'a1' must have constant destruction"
//@ diag: 20:45 note "a constant expression cannot modify an object that is visible outside that expression"
//@ diag: 22:13 note "in call to 'a1.~A()'"
//@ diag: 28:13 error "constexpr variable 'arr' must have constant destruction"
//@ diag: 20:45 note "a constant expression cannot modify an object that is visible outside that expression"
//@ diag: 28:13 note "in call to 'arr[1].~A()'"
//@ diag: 36:13 error "constexpr variable 'y1' must have constant destruction"
//@ diag: 34:47 note "subexpression not valid in a constant expression"
//@ diag: 36:13 note "in call to 'y1.~Y()'"
//@ diag: 37:33 error "constexpr variable 'yy' must have constant destruction"
//@ diag: 34:47 note "subexpression not valid in a constant expression"
//@ diag: 37:33 note "in call to 'yy.~Y()'"
int g;
struct A { int n; constexpr ~A() { if (n) g = 1; } };
constexpr A a0{0};
constexpr A a1{1};
struct B { int *p; constexpr B() : p(new int(1)) {} constexpr ~B() { delete p; } };
constexpr int ok() { B b; return *b.p; }
static_assert(ok() == 1);
struct D { constexpr ~D() {} };
constexpr D d0;
constexpr A arr[2] = {{0}, {1}};
struct M { int n; constexpr ~M() { n = 7; } };
constexpr M m{1};
static_assert(m.n == 1);
struct N { M a; M b; int k; constexpr ~N() { k = a.n + b.n; } };
constexpr N nn{{1}, {2}, 3};
struct Y { int v; constexpr ~Y() { if (v > 2) throw 1; } };
constexpr Y y0{1};
constexpr Y y1{3};
constexpr int f() { constexpr Y yy{5}; return 1; }
struct Z { int *p; constexpr ~Z() { delete p; } };
constexpr Z z{nullptr};
