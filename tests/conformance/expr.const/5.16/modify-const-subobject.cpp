// [expr.const]/5.16, [dcl.type.cv]/4: a const subobject of a non-const
// object (a const member, an element of a const array member, a const
// member reached through a base or a union member, a non-mutable member
// of a const object) is a const object, and modifying it (assignment,
// `++`, compound assignment) is not a constant expression; its own
// constructor and destructor may write it ([class.ctor.general]/5,
// [class.dtor]/5), and a `mutable` member of a const object stays
// modifiable. Found running GCC's C++ testsuite (WP-1468).
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 62:15 error "static assertion expression is not an integral constant expression"
//@ diag: 61:56 note "modification of object of const-qualified type 'const int' is not allowed in a constant expression"
//@ diag: 62:15 note "in call to 'member()'"
//@ diag: 64:15 error "static assertion expression is not an integral constant expression"
//@ diag: 56:60 note "modification of object of const-qualified type 'const int' is not allowed in a constant expression"
//@ diag: 63:26 note "in call to 'G()'"
//@ diag: 64:15 note "in call to 'body()'"
//@ diag: 66:15 error "static assertion expression is not an integral constant expression"
//@ diag: 55:69 note "modification of object of const-qualified type 'const int' is not allowed in a constant expression"
//@ diag: 65:28 note "in call to 'F()'"
//@ diag: 66:15 note "in call to 'scalar()'"
//@ diag: 69:15 error "static assertion expression is not an integral constant expression"
//@ diag: 68:58 note "modification of object of const-qualified type 'const int' is not allowed in a constant expression"
//@ diag: 69:15 note "in call to 'element()'"
//@ diag: 74:15 error "static assertion expression is not an integral constant expression"
//@ diag: 73:56 note "modification of object of const-qualified type 'const int' is not allowed in a constant expression"
//@ diag: 74:15 note "in call to 'mut2()'"
//@ diag: 77:15 error "static assertion expression is not an integral constant expression"
//@ diag: 76:52 note "modification of object of const-qualified type 'const int' is not allowed in a constant expression"
//@ diag: 77:15 note "in call to 'base()'"
//@ diag: 78:15 error "constexpr function never produces a constant expression"
//@ diag: 78:58 note "modification of object of const-qualified type 'const int' is not allowed in a constant expression"
//@ diag: 79:15 error "static assertion expression is not an integral constant expression"
//@ diag: 78:58 note "modification of object of const-qualified type 'const int' is not allowed in a constant expression"
//@ diag: 79:15 note "in call to 'un()'"
//@ diag: 81:15 error "static assertion expression is not an integral constant expression"
//@ diag: 80:30 note "modification of object of const-qualified type 'const int' is not allowed in a constant expression"
//@ diag: 81:15 note "in call to 'inc()'"
//@ diag: 83:15 error "static assertion expression is not an integral constant expression"
//@ diag: 82:60 note "modification of object of const-qualified type 'const int' is not allowed in a constant expression"
//@ diag: 83:15 note "in call to 'compound()'"
//@ diag: 86:15 error "static assertion expression is not an integral constant expression"
//@ diag: 54:67 note "modification of object of const-qualified type 'const int' is not allowed in a constant expression"
//@ diag: 85:26 note "in call to 'e.~E()'"
//@ diag: 86:15 note "in call to 'dtor()'"
struct A { int n; constexpr A() : n(1) { n = 2; } };
struct B { const A a; int m = 0; };
struct C { const int x[2] = {1, 2}; };
struct M { mutable int m = 0; int k = 0; };
struct Base { const int b = 1; };
struct D : Base { int d = 0; };
union U { const int c; int i; };
struct E { const int e = 1; constexpr ~E() { const_cast<int &>(e) = 2; } };
struct F { const int f; constexpr F() : f(1) { const_cast<int &>(f) = 2; } };
struct G { const A a; constexpr G() { const_cast<A &>(a).n = 3; } };

// A const member's own constructor may write it; a later write may not.
constexpr int own() { B b{}; return b.a.n; }
static_assert(own() == 2);
constexpr int member() { B b{}; const_cast<A &>(b.a).n = 3; return 0; }
static_assert(member() == 0);
constexpr int body() { G g; return 0; }
static_assert(body() == 0);
constexpr int scalar() { F f; return f.f; }
static_assert(scalar() == 0);
// An element of a const array member.
constexpr int element() { C c; const_cast<int &>(c.x[1]) = 0; return 0; }
static_assert(element() == 0);
// A mutable member of a const object is modifiable, its other members are not.
constexpr int mut() { const M m; m.m = 4; return m.m; }
static_assert(mut() == 4);
constexpr int mut2() { const M m; const_cast<M &>(m).k = 4; return 0; }
static_assert(mut2() == 0);
// Through a base-class path, a union member, ++ and compound assignment.
constexpr int base() { D d; const_cast<int &>(d.b) = 2; return 0; }
static_assert(base() == 0);
constexpr int un() { U u{.c = 1}; const_cast<int &>(u.c) = 2; return 0; }
static_assert(un() == 0);
constexpr int inc() { B b{}; ++const_cast<int &>(b.a.n); return 0; }
static_assert(inc() == 0);
constexpr int compound() { B b{}; const_cast<int &>(b.a.n) += 2; return 0; }
static_assert(compound() == 0);
// A const member's destructor runs on a modifiable object.
constexpr int dtor() { E e; return e.e; }
static_assert(dtor() == 1);
// A non-const member next to a const one stays modifiable.
constexpr int other() { B b{}; b.m = 5; return b.m; }
static_assert(other() == 5);
