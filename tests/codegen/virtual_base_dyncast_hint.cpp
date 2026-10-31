// The `-1` `__dynamic_cast` offset hint: the source is reached from the
// destination only through a virtual base (clang `computeOffsetHint`).
struct V { int v = 1; virtual ~V() noexcept {} };
struct W { long w = 2; virtual void fw() noexcept {} };
struct A : virtual V { int a = 3; };
struct B : virtual V, W { int b = 4; };
struct D : A, B { int d = 5; };
struct E : virtual W { int e = 6; };
A* down(V* p) noexcept { return dynamic_cast<A*>(p); }
D* down2(V* p) noexcept { return dynamic_cast<D*>(p); }
E* down3(W* p) noexcept { return dynamic_cast<E*>(p); }
B* cross(A* p) noexcept { return dynamic_cast<B*>(p); }
D* from_w(W* p) noexcept { return dynamic_cast<D*>(p); }
