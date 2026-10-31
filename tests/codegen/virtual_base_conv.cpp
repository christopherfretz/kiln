// Conversions to virtual bases (clang GetAddressOfBaseClass): the vbase
// offset from the vtable, null checks, a non-virtual tail after the
// virtual step, a final class (static offset), lvalues.
struct V { int v; };
struct W { long w; };
struct L : virtual V { int l; };
struct R : virtual V, virtual W { int r; };
struct D : L, R { int d; };
struct F final : virtual V { int f; };
struct X { int x; };
struct M : X, virtual W {};
struct N : virtual M {};
V *to_v(L *p) noexcept { return p; }
V *this_like(D *p) noexcept { return static_cast<L *>(p); }
V *dv(D *p) noexcept { return p; }
W &dw(D &r) noexcept { return r; }
int dvr(D &r) noexcept { return r.v; }
V *fv(F *p) noexcept { return p; }
X *nx(N *p) noexcept { return p; }
W *nw(N *p) noexcept { return p; }
