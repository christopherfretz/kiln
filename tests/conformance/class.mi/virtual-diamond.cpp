// [class.mi]/4-6: a virtual base is shared by every base-specifier naming it
// virtual, so its members and conversions to it are unambiguous.
//@ expect: compile-ok
//@ strict-diags
struct V { int v; static int s; };
struct L : virtual V { int l; };
struct R : virtual V { int r; };
struct D : L, R { int d; };
struct E : virtual L, virtual R, virtual V {};
D d;
E e;
V *pv = &d;
L *pl = &d;
V *pv2 = &e;
int g() { return d.v + e.v + d.l + d.r + D::s + e.l; }
