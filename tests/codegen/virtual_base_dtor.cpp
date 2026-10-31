// Destructors of classes with virtual bases: D1 calls D2 with the class's
// own VTT and then destroys the virtual bases (reverse vbases() order);
// D2 takes the VTT and skips them; a virtual base with virtual bases of
// its own (sub-VTT), fields, a final class, and a vtable whose address
// point is at its end (no virtual functions in the primary vtable).
int n;
struct M { int m; ~M() { n += m; } };
struct A { int a; ~A() { ++n; } };
struct B : virtual A { M mb{3}; virtual void h() {} ~B() { n *= 2; } };
struct C : virtual B { int c; ~C() { --n; } };
struct V { int v; V(int x) noexcept : v(x) {} virtual ~V() { ++n; } };
struct L : virtual V { int l = 2; L() noexcept : V(10) {} ~L() { ++n; } };
struct R : virtual V, virtual A { int r = 3; R() noexcept : V(20) {} ~R() { ++n; } };
struct D : L, R { M md{4}; D() noexcept : V(5) {} ~D() { ++n; } };
struct E : D, virtual C { E() noexcept : V(7) {} };
struct F final : virtual A { ~F() {} };
int get() noexcept { { C c; } { E e; } { F f; } return n; }
