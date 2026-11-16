// [class.cdtor]/4-6: during construction and destruction the dynamic type
// is the class of the running constructor or destructor, for a virtual
// call (from the body, a member initializer or a base's constructor
// through a pointer to the complete object), `dynamic_cast` and `typeid`.
// Found running GCC's C++ testsuite (WP-1468).
//@ std: c++23
//@ expect: compile-fail
//@ strict-diags
//@ diag: 54:15 error "static assertion expression is not an integral constant expression"
//@ diag: 52:35 note "subexpression not valid in a constant expression"
//@ diag: 51:40 note "in call to 'R(&q)'"
//@ diag: 53:28 note "in call to 'Q()'"
//@ diag: 54:15 note "in call to 'notyet()'"
//@ diag: 60:15 error "static assertion expression is not an integral constant expression"
//@ diag: 58:35 note "subexpression not valid in a constant expression"
//@ diag: 57:32 note "in call to 'T(&s)'"
//@ diag: 59:29 note "in call to 'S()'"
//@ diag: 60:15 note "in call to 'notyet2()'"
#include <typeinfo>
struct B {
  int seen = 0, cast = 0, ty = 0, dseen = 0;
  int *out = nullptr;
  constexpr virtual int f() const { return 1; }
  constexpr B() { seen = f(); cast = dynamic_cast<const B *>(this) != nullptr; }
  constexpr virtual ~B() { if (out) *out = f(); }
};
struct D : B {
  int m = f();
  constexpr int f() const override { return 2; }
  constexpr D() {}
  constexpr ~D() override {}
};
constexpr int ctor_body() { D d; return d.seen; }
static_assert(ctor_body() == 1);
constexpr int member_init() { D d; return d.m; }
static_assert(member_init() == 2);
constexpr int after() { D d; B &b = d; return b.f(); }
static_assert(after() == 2);
constexpr int dtor() { int r = 0; { D d; d.out = &r; } return r; }
static_assert(dtor() == 1);
// dynamic_cast to the derived class during the base's construction fails.
struct X;
struct Y { constexpr virtual ~Y() {} int ok = 0; constexpr Y(); };
struct X : Y { constexpr X() {} };
constexpr Y::Y() { ok = dynamic_cast<X *>(this) == nullptr; }
constexpr int down() { X x; return x.ok; }
static_assert(down() == 1);
struct P { constexpr virtual int g() const { return 1; } };
struct Q;
struct R { int v; constexpr R(const Q *q); };
struct Q : P, R { constexpr Q() : P(), R(this) {} constexpr int g() const override { return 3; } };
constexpr R::R(const Q *q) : v(q->g()) {}
constexpr int notyet() { Q q; return q.v; }
static_assert(notyet() == 1);
struct S; 
struct T { int v; constexpr T(const S *s); constexpr virtual int h() const { return 5; } };
struct S : T { constexpr S() : T(this) {} constexpr int h() const override { return 6; } };
constexpr T::T(const S *s) : v(s->h()) {}
constexpr int notyet2() { S s; return s.v; }
static_assert(notyet2() == 5);
struct V; 
struct W { int v; constexpr W(V *s); constexpr virtual ~W() {} };
struct V : W { constexpr V() : W(this) {} };
constexpr W::W(V *s) : v(dynamic_cast<V *>(static_cast<W*>(s)) == nullptr) {}
constexpr int dc() { V s; return s.v; }
static_assert(dc() == 1);
struct TB { bool same = false; constexpr TB(); constexpr virtual ~TB() {} };
struct TD : TB { constexpr TD() {} };
constexpr TB::TB() { same = typeid(*this) == typeid(TB); }
constexpr bool tid() { TD d; return d.same && typeid(d) == typeid(TD); }
static_assert(tid());
