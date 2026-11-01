// User-defined conversions (WP-204): implicit and explicit conversion
// functions, explicit `a.operator T()` member calls (also through a
// pointer and in a template), class-to-class conversions that construct
// a temporary, and aggregate elements initialized through a conversion
// function rather than by brace elision ([dcl.init.aggr]/4.2).
namespace scalar {
struct A { int v; operator int() const { return v; } explicit operator bool() const { return v != 0; } };
struct B { int x, y; };
struct C { operator B() const { return B{1, 2}; } };
struct R { int g; operator int&() { return g; } };
int f1(A a) { return a; }
int f2(A a) { return a.operator int(); }
bool f3(A a) { if (a) return true; return static_cast<bool>(a); }
B f4(C c) { B b = c; return b; }
int f5(R r) { int& x = r; x = 3; return r.operator int&(); }
long f6(A a) { return a + 1L; }
} // namespace scalar

namespace classes {
struct B { int x, y; ~B() {} };
struct C { operator B() const { return B{1, 2}; } };
template <class T> struct W { T v; operator T() const { return v; } template <class U> operator U*() const { return nullptr; } };
struct D { B b; };
int use(B);
int g1(C c) { return use(c); }
int g2(W<int> w) { int* p = w; return w.operator int() + (p ? 1 : 0); }
int g3(W<int>* w) { return w->operator int(); }
B g4(C c) { return c; }
D g5(C c) { return D{B(c)}; }
B g6(C c) { return B(c); }
B g7(C c) { return static_cast<B>(c); }
B g8(C c) { return c.operator B(); }
template <class T> int g9(T t) { return t.operator int(); }
int g10() { return g9(W<int>{3}); }
} // namespace classes

namespace aggregates {
struct B { int x, y; ~B() {} };
struct C { operator B() const { return B{1, 2}; } };
struct D { B b; int z; };
struct E { B arr[2]; };
D h1(C c) { return D{c, 3}; }
D h2(C c) { return {c}; }
E h3(C c) { return E{c, c}; }
D h4(int i) { return D{i, 2, 3}; }
} // namespace aggregates
