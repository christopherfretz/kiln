// [class.mi]/4-6 (a virtual base is shared), [class.virtual]/2, Itanium C++
// ABI §2.5.2 (vbase and vcall offsets), §2.6 (virtual thunks), §2.6.2
// (VTT), §2.6.3 (construction vtables for L-in-D and R-in-D).
//@ expect: vtables
struct V { virtual void g(); virtual void h(); int v; };
struct L : virtual V { void g() override; int l; };
struct R : virtual V { void h() override; virtual void m(); int r; };
struct D : L, R { void g() override; virtual void k(); };
void V::g() {} void V::h() {} void L::g() {} void R::h() {} void R::m() {}
void D::g() {} void D::k() {}
