// [dcl.fct.def.default]/5, [class.default.ctor]/4, [expr.const]/5.2: a
// constexpr default constructor defaulted out of line (also in a class
// template) is defined there and usable in a constant expression. One
// that can never be constant is not diagnosed at its definition even
// under -Winvalid-constexpr (clang runs `isPotentialConstantExpr` only on
// written bodies, never on implicit or defaulted ones); its use is, with
// the default member initializer's note and a call note.
//@ std: c++20
//@ args: -Winvalid-constexpr
//@ expect: compile-fail
//@ strict-diags
//@ diag: 53:13 error "constexpr variable 's1' must be initialized by a constant expression"
//@ diag: 44:19 note "non-constexpr function 'g' cannot be used in a constant expression"
//@ diag: 53:13 note "in call to 'S()'"
//@ diag: 36:5 note "declared here"
//@ diag: 54:15 error "constexpr variable 'u1' must be initialized by a constant expression"
//@ diag: 45:19 note "non-constexpr function 'g' cannot be used in a constant expression"
//@ diag: 54:20 note "in call to 'T()'"
//@ diag: 36:5 note "declared here"
//@ diag: 55:13 error "constexpr variable 'r1' must be initialized by a constant expression"
//@ diag: 46:20 note "non-constexpr function 'g' cannot be used in a constant expression"
//@ diag: 55:13 note "in call to 'R()'"
//@ diag: 36:5 note "declared here"
//@ diag: 56:13 error "constexpr variable 'z1' must be initialized by a constant expression"
//@ diag: 47:20 note "non-constexpr function 'g' cannot be used in a constant expression"
//@ diag: 56:13 note "in call to 'Z()'"
//@ diag: 36:5 note "declared here"
//@ diag: 57:18 error "constexpr variable 'q1' must be initialized by a constant expression"
//@ diag: 49:37 note "non-constexpr function 'g' cannot be used in a constant expression"
//@ diag: 57:18 note "in call to 'Q()'"
//@ diag: 36:5 note "declared here"
//@ diag: 58:18 error "constexpr variable 'p1' must be initialized by a constant expression"
//@ diag: 50:37 note "non-constexpr function 'g' cannot be used in a constant expression"
//@ diag: 58:18 note "in call to 'P()'"
//@ diag: 36:5 note "declared here"
int g();
struct A { int x = 1; constexpr A(); };
constexpr A::A() = default;
static_assert(A().x == 1);
template <class T> struct B { T x = 2; constexpr B(); };
template <class T> constexpr B<T>::B() = default;
constexpr int h() { B<int> b; return b.x; }
static_assert(h() == 2);
class S { int x = g(); public: int y = 0; };
class T { int x = g(); public: T() = default; int y = 0; };
struct R { int x = g(); constexpr R() = default; int y = 0; };
struct Z { int x = g(); constexpr Z(); int y = 0; };
constexpr Z::Z() = default;
template <class X> struct Q { X x = g(); constexpr Q() = default; };
template <class X> struct P { X x = g(); constexpr P(); };
template <class X> constexpr P<X>::P() = default;
S s0; T t0; R r0; Z z0; Q<int> q0; P<int> p0;
constexpr S s1;
constexpr int u1 = T().y;
constexpr R r1;
constexpr Z z1;
constexpr Q<int> q1;
constexpr P<int> p1;
