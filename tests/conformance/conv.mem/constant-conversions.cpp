// [conv.mem]/2, [expr.static.cast]/12, [expr.mptr.oper]: member pointer
// conversions along base paths (including a second base and an anonymous
// member) and accesses through them are constant expressions
//@ expect: compile-ok
//@ strict-diags
struct A { int a = 1; constexpr int f() const { return a + 10; } virtual constexpr int g() const { return 2; } };
struct B : A { int b = 3; };
struct C : B { int c = 4; constexpr int g() const override { return 5; } };
struct D { int d = 6; };
struct E : C, D { int e = 7; };
struct U { union { int x; float y; }; struct { int z; }; };
constexpr int A::* p1 = &A::a;
constexpr int C::* p2 = p1;
constexpr int A::* p3 = static_cast<int A::*>(p2);
static_assert(p3 == p1);
constexpr int E::* p4 = &D::d;
constexpr int D::* p5 = static_cast<int D::*>(p4);
static_assert(p5 == &D::d);
constexpr E e{};
static_assert(e.*p4 == 6 && e.*p2 == 1);
static_assert(&((&e)->*p4) == &e.d);
constexpr int (A::* f1)() const = &A::f;
constexpr int (E::* f2)() const = f1;
static_assert((e.*f2)() == 11);
constexpr int (C::* g2)() const = &A::g;
static_assert((C{}.*g2)() == 5);
constexpr int (A::* g3)() const = static_cast<int (A::*)() const>(g2);
static_assert((A{}.*g3)() == 2);
constexpr U u{.x = 8};
static_assert(u.*(&U::x) == 8);
constexpr int B::* q = &B::b;
constexpr int A::* q2 = static_cast<int A::*>(q);
constexpr int C::* q3 = q2;
static_assert(q3 == &C::b && C{}.*q3 == 3);
constexpr int B::* nb = nullptr;
static_assert(static_cast<int A::*>(nb) == nullptr && !nb);
