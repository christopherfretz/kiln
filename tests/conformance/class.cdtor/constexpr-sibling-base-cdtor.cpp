// [class.cdtor]/4-6: while a constructor or destructor of a base class
// subobject `x` runs, a virtual call, `typeid` or `dynamic_cast` applied
// to another base of the object `x` belongs to (a sibling reached through
// a pointer the structor was given, also from a member's constructor
// running inside `x`'s) is undefined, so not a constant expression; the
// same operations on `x` itself, its bases, the enclosing object from a
// member's constructor, or after construction has finished are fine.
// clang 22 and GCC 16.1 accept the undefined calls: the wording is kiln's
// own (spec/oven.md "Dynamic types"). Found running GCC's C++ testsuite
// (WP-1494).
//@ skip(clang): clang 22 does not diagnose [class.cdtor]/4-6 sibling calls
//@ std: c++26
//@ expect: compile-fail
//@ strict-diags
//@ diag: 66:15 error "static assertion expression is not an integral constant expression"
//@ diag: 54:26 note "virtual function called on subobject of type 'A' that is not the object of type 'B' under construction or one of its bases"
//@ diag: 60:31 note "in call to 'B(&d, &d, 2)'"
//@ diag: 63:32 note "in call to 'D(2)'"
//@ diag: 66:15 note "in call to 'run(2)'"
//@ diag: 51:13 note "object of type 'B' is under construction here"
//@ diag: 67:15 error "static assertion expression is not an integral constant expression"
//@ diag: 55:23 note "typeid applied to subobject of type 'A' that is not the object of type 'B' under construction or one of its bases"
//@ diag: 60:31 note "in call to 'B(&d, &d, 3)'"
//@ diag: 63:32 note "in call to 'D(3)'"
//@ diag: 67:15 note "in call to 'run(3)'"
//@ diag: 51:13 note "object of type 'B' is under construction here"
//@ diag: 68:15 error "static assertion expression is not an integral constant expression"
//@ diag: 56:23 note "dynamic_cast applied to subobject of type 'A' that is not the object of type 'B' under construction or one of its bases"
//@ diag: 60:31 note "in call to 'B(&d, &d, 4)'"
//@ diag: 63:32 note "in call to 'D(4)'"
//@ diag: 68:15 note "in call to 'run(4)'"
//@ diag: 51:13 note "object of type 'B' is under construction here"
//@ diag: 81:15 error "static assertion expression is not an integral constant expression"
//@ diag: 78:88 note "virtual function called on subobject of type 'A' that is not the object of type 'G' under destruction or one of its bases"
//@ diag: 80:43 note "in call to 'h.~G()'"
//@ diag: 80:43 note "in call to 'h.~H()'"
//@ diag: 81:15 note "in call to 'dtor_sib()'"
//@ diag: 78:64 note "object of type 'G' is under destruction here"
//@ diag: 88:15 error "static assertion expression is not an integral constant expression"
//@ diag: 84:50 note "virtual function called on subobject of type 'A' that is not the object of type 'P2' under construction or one of its bases"
//@ diag: 85:45 note "in call to 'N(&k)'"
//@ diag: 86:41 note "in call to 'P2(&k)'"
//@ diag: 87:32 note "in call to 'K()'"
//@ diag: 88:15 note "in call to 'member_sib()'"
//@ diag: 85:28 note "object of type 'P2' is under construction here"
#include <typeinfo>
struct A { virtual constexpr int f() const { return 1; } };
struct V { virtual constexpr int g() const { return 2; } };
struct B : V {
  int r = 0;
  constexpr B(const A *a, const V *v, int how) {
    if (how == 0) r = v->g();          // own base: fine
    if (how == 1) r = this->g();       // itself: fine
    if (how == 2) r = a->f();          // sibling: UB
    if (how == 3) r = typeid(*a) == typeid(A); // sibling: UB
    if (how == 4) r = dynamic_cast<const void *>(a) != nullptr; // sibling: UB
  }
};
struct D : A, B {
  constexpr D(int how) : A(), B(this, this, how) {}
  constexpr int f() const override { return 3; }
};
constexpr int run(int how) { D d(how); return d.r; }
static_assert(run(0) == 2);
static_assert(run(1) == 2);
static_assert(run(2) == 1);
static_assert(run(3) == 1);
static_assert(run(4) == 1);
constexpr int after() { D d(0); const A *a = &d; return a->f() + (typeid(*a) == typeid(D)) + (dynamic_cast<const B *>(a) != nullptr); }
static_assert(after() == 5);
// A member under construction calling back into the enclosing object.
struct E;
struct M { int r; constexpr M(const E *e); };
struct E : A { M m; constexpr E() : m(this) {} constexpr int f() const override { return 7; } };
constexpr M::M(const E *e) : r(static_cast<const A *>(e)->f()) {}
static_assert(E().m.r == 7);
// Destruction: a sibling reached from a base's destructor.
struct G { const A *a = nullptr; int *out = nullptr; constexpr ~G() { if (a) *out = a->f(); } };
struct H : A, G { constexpr H(int *o) { a = this; out = o; } };
constexpr int dtor_sib() { int r = 0; { H h(&r); } return r; }
static_assert(dtor_sib() == 1);
// A member's constructor calling back into the enclosing object's base under construction.
struct K;
struct N { int r; constexpr N(const A *a) : r(a->f()) {} };
struct P2 { N n; constexpr P2(const A *a) : n(a) {} };
struct K : A, P2 { constexpr K() : A(), P2(this) {} };
constexpr int member_sib() { K k; return k.n.r; }
static_assert(member_sib() == 1);
// dynamic_cast / typeid / call through the structor's own bases are fine.
struct Q2 : V { int r = 0; constexpr Q2() { const V *v = this; r = (typeid(*v) == typeid(Q2)) + (dynamic_cast<const Q2 *>(v) != nullptr) + v->g(); } };
static_assert(Q2().r == 4);
