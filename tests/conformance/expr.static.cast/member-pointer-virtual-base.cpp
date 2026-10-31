// [conv.mem]/2, [expr.static.cast]/12: a pointer-to-member conversion,
// implicit or by static_cast / C-style cast (in either direction), must
// not go through a virtual base, even one in the middle of the path.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:33 error "conversion from pointer to member of class 'W' to pointer to member of class 'Y' via virtual base 'X' is not allowed"
//@ diag: 16:33 error "conversion from pointer to member of class 'Y' to pointer to member of class 'W' via virtual base 'X' is not allowed"
//@ diag: 17:33 error "conversion from pointer to member of class 'Y' to pointer to member of class 'W' via virtual base 'X' is not allowed"
//@ diag: 18:43 error "conversion from pointer to member of class 'W' to pointer to member of class 'Y' via virtual base 'X' is not allowed"
//@ diag: 20:33 error "conversion from pointer to member of class 'V' to pointer to member of class 'D' via virtual base 'V' is not allowed"
//@ diag: 20:70 error "conversion from pointer to member of class 'D' to pointer to member of class 'V' via virtual base 'V' is not allowed"
struct W { int w; void fw(); };
struct X : W { int x; };
struct Y : virtual X { int y; };
int Y::* a(int W::* p) { return p; }
int W::* b(int Y::* p) { return static_cast<int W::*>(p); }
int W::* c(int Y::* p) { return (int W::*)p; }
void (Y::* d(void (W::* p)()))() { return p; }
struct V { int v; }; struct B : virtual V { int b; }; struct D : B { int d; };
int D::* e(int V::* p) { return p; } int V::* f(int D::* p) { return static_cast<int V::*>(p); }
