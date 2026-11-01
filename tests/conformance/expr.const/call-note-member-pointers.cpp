// [expr.const]/5, [expr.unary.op]/4 (WP-241): a pointer-to-member argument
// is printed in the "in call to" note as `&C::m` (the member's class, also
// for a base member converted to a derived class's member pointer), and
// the null member pointer as `0`, as clang does.
//@ expect: compile-fail
//@ strict-diags
//@ args: -Wno-division-by-zero
//@ diag: 38:15 error "static assertion expression is not an integral constant expression"
//@ diag: 32:38 note "division by zero"
//@ diag: 38:15 note "in call to 'f6(&M::y)'"
//@ diag: 39:15 error "static assertion expression is not an integral constant expression"
//@ diag: 32:38 note "division by zero"
//@ diag: 39:15 note "in call to 'f6(0)'"
//@ diag: 40:15 error "static assertion expression is not an integral constant expression"
//@ diag: 33:38 note "division by zero"
//@ diag: 40:15 note "in call to 'f7(&M::x)'"
//@ diag: 41:15 error "static assertion expression is not an integral constant expression"
//@ diag: 34:48 note "division by zero"
//@ diag: 41:15 note "in call to 'f8(&M::g)'"
//@ diag: 42:15 error "static assertion expression is not an integral constant expression"
//@ diag: 36:31 note "division by zero"
//@ diag: 42:15 note "in call to 'f9({&M::x, 1})'"
//@ diag: 43:15 error "static assertion expression is not an integral constant expression"
//@ diag: 36:31 note "division by zero"
//@ diag: 43:15 note "in call to 'f9({0, 0})'"
//@ diag: 44:15 error "static assertion expression is not an integral constant expression"
//@ diag: 37:44 note "division by zero"
//@ diag: 44:15 note "in call to 'fa(&TT::q)'"
struct M { int x; int y; constexpr int g() const { return 0; } };
struct N : M { int z; };
template<class T> struct TT { int q; };
constexpr int f6(int M::*) { return 1/0; }
constexpr int f7(int N::*) { return 1/0; }
constexpr int f8(int (M::*)() const) { return 1/0; }
struct H { int M::* p; int v; };
constexpr int f9(H) { return 1/0; }
constexpr int fa(int TT<int>::*) { return 1/0; }
static_assert(f6(&M::y));
static_assert(f6(nullptr));
static_assert(f7(&M::x));
static_assert(f8(&M::g));
static_assert(f9(H{&M::x, 1}));
static_assert(f9(H{}));
static_assert(fa(&TT<int>::q));
