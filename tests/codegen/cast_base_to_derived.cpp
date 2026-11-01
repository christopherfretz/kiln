// [expr.static.cast]/2, /11: a static_cast from a base class pointer or
// reference to a derived class one subtracts the base's offset; a pointer
// is null-checked (clang `GetAddressOfDerivedClass`).
struct A { int a; };
struct B { long b; };
struct C : A, B { int c; };
struct E : C { char e; };
struct alignas(32) F : A, B { int f; };
C* p1(B* b) { return static_cast<C*>(b); }
C* p2(A* a) { return static_cast<C*>(a); }
E* p3(B* b) { return (E*)b; }
C& r1(B& b) { return static_cast<C&>(b); }
int r2(B& b) { return static_cast<C&>(b).c; }
int r3(B&& b) { return static_cast<C&&>(b).c; }
const C* p4(const B* b) { return static_cast<const C*>(b); }
int p5(B* b) { return static_cast<C*>(b)->c; }
int p6(B* b) { return static_cast<F*>(b)->f; }
struct G : B { void g(); int x; };
void B2G(B* b) { static_cast<G*>(b)->g(); }
struct H : A, B { int h(); int x; };
int H::h() { B* b = this; return static_cast<H*>(b)->x; }
struct B3 { long z; void f(); };
struct D3 : A, B3 { int y; };
void B3::f() { static_cast<D3*>(this)->y = 1; static_cast<D3&>(*this).y = 2; }
