//@ expect: vtables
struct V { virtual void g(); int v; };
struct L : virtual V { void g(); int l; };
struct R : virtual V { int r; virtual void m(); };
struct D : L, R { virtual void k(); };
void V::g() {} void L::g() {} void R::m() {} void D::k() {}
