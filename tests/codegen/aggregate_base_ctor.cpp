// Aggregate base elements built by the base-object (C2) constructor
// (WP-170; clang `EK_Base` -> `CXXConstructExpr::NonVirtualBase`): a
// copy or move from an lvalue/xvalue of the base or a derived class, a
// braced list or value-initialization of a non-aggregate base, and an
// omitted base. An elided prvalue `B()` and a converting constructor's
// temporary (`D{5, 6}`) stay complete-object (C1). A value-initialized
// base subobject is zeroed through a byte GEP over its non-virtual size
// (clang `EmitNullBaseClassInitialization`), also for a mem-initializer.
struct M { M(); M(const M&); int m; };
struct B { int b; B(); B(const B&); B(int); B(int, int); };
struct X : B { int x; };
struct D : B { int d; };
D f1(const B &b) { return D{b, 1}; }
D f2() { return D{{}, 2}; }
D f3() { return D{B(), 3}; }
D f4() { return D{}; }
D f5() { return D{5, 6}; }
D f6(B &&b) { return D{static_cast<B&&>(b), 7}; }
D f7(const X &x) { return D{x, 1}; }
D f8() { return D{{5}, 2}; }
D f9() { return D{{5, 6}, 2}; }
struct E : B { int e = 4; };
E f10() { return E{}; }
E f11(B b) { return E{b}; }
struct A { M m; int a; };
struct F : A { int f; };
F f12() { return F{}; }
F f13(const A &a) { return F{a, 1}; }
F f14() { return F{{}, 1}; }
F f15() { return F{{M(), 2}, 1}; }
class P { M m; int p; public: int q; };
struct G : P { int g; };
G f16() { return G{}; }
G f17() { return G{{}, 2}; }
struct G2 : P { int z; G2() : P() {} };
void f18() { G2 g; }
struct V { virtual void f(); int v; };
struct W : V { int w; W() : V() {} };
void f19() { W w; }
template<class T> struct TB { T t; TB(); TB(const TB&); };
struct H : TB<int> { int h; };
H f20(const TB<int> &t) { return H{t, 1}; }
H f21() { return H{}; }
struct I : B, A { int i; };
I f22(const B &b, const A &a) { return I{b, a, 1}; }
I f23() { return I{}; }
