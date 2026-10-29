// [expr.mptr.oper]/2-6, [conv.mem]/2, [expr.static.cast]/12, [expr.eq]/4:
// pointers to data members and member functions in constant evaluation,
// through base-to-derived and derived-to-base conversions, in constexpr
// variables and class members, and compared.
struct A { int a = 1; constexpr int fa() const { return a * 10; } };
struct B { int b = 2; constexpr int fb() const { return b * 10; } constexpr int add(int k) const { return b + k; } };
struct D : A, B { int d = 3; };
struct E : D { int e = 4; };

constexpr int read() { D x; int D::*p = &D::d; return x.*p; }
static_assert(read() == 3);
constexpr int write() { D x; D *px = &x; int D::*p = &D::d; px->*p = 7; return x.d; }
static_assert(write() == 7);
constexpr int base_member() { int D::*p = &B::b; D x; return x.*p; }
static_assert(base_member() == 2);
constexpr int two_steps() { int E::*p = &B::b; E x; x.*p = 9; return x.b; }
static_assert(two_steps() == 9);
constexpr int round_trip() { int D::*p = &B::b; int B::*q = static_cast<int B::*>(p); B y; return y.*q; }
static_assert(round_trip() == 2);
constexpr int to_base() { D x; int A::*q = static_cast<int A::*>(&D::d); A &r = x; return r.*q; }
static_assert(to_base() == 3);
constexpr int call() { int (D::*pf)() const = &B::fb; D x; return (x.*pf)(); }
static_assert(call() == 20);
constexpr int call_arrow() { const D x; const D *px = &x; int (D::*pf)(int) const = &B::add; return (px->*pf)(5); }
static_assert(call_arrow() == 7);
constexpr int call_derived() { int (E::*pf)() const = &A::fa; const E x; return (x.*pf)(); }
static_assert(call_derived() == 10);

constexpr int D::*global = &B::b;
constexpr int use_global() { D x; return x.*global; }
static_assert(use_global() == 2);
struct H { int D::*m; int (D::*f)() const; };
constexpr H h{&A::a, &A::fa};
constexpr int use_member() { D x; return x.*(h.m) + (x.*(h.f))(); }
static_assert(use_member() == 11);

constexpr int compare() {
  int D::*p = &D::d, D::*q = nullptr;
  int D::*r = &B::b;
  return (p == &D::d) + 2 * (p != r) + 4 * (q == nullptr) + 8 * !q + 16 * !!p;
}
static_assert(compare() == 31);
