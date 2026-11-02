// [dcl.constexpr]/6, [expr.const]/7: a constexpr variable must have
// constant destruction: its destruction, evaluated at the end of the
// initialization, must be a constant expression. The destructor runs with
// the object no longer const (it may modify the members).
// Pinned to C++23: in C++26 a caught or uncaught throw is evaluated (P3068).
//@ std: c++23
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:13 error "constexpr variable 'a1' must have constant destruction"
//@ diag: 22:45 note "a constant expression cannot modify an object that is visible outside that expression"
//@ diag: 24:13 note "in call to 'a1.~A()'"
//@ diag: 30:13 error "constexpr variable 'arr' must have constant destruction"
//@ diag: 22:45 note "a constant expression cannot modify an object that is visible outside that expression"
//@ diag: 30:13 note "in call to 'arr[1].~A()'"
//@ diag: 38:13 error "constexpr variable 'y1' must have constant destruction"
//@ diag: 36:47 note "subexpression not valid in a constant expression"
//@ diag: 38:13 note "in call to 'y1.~Y()'"
//@ diag: 39:33 error "constexpr variable 'yy' must have constant destruction"
//@ diag: 36:47 note "subexpression not valid in a constant expression"
//@ diag: 39:33 note "in call to 'yy.~Y()'"
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
