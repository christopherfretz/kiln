// Constructors of classes with virtual bases: separate C1/C2, the VTT
// (`_ZTT`) and construction vtables (`_ZTC`), the VTT parameter of the
// base-object variant, vptr stores from the VTT, a diamond, thunks with
// a virtual adjustment (`_ZTv`), and a vbase reached through a
// non-virtual base.
struct V {
  int v;
  V(int x) noexcept : v(x) {}
  virtual int f() noexcept { return v; }
};
struct L : virtual V {
  int l = 2;
  L() noexcept : V(10) {}
  int f() noexcept override { return l + v; }
};
struct R : virtual V {
  int r = 3;
  R() noexcept : V(20) {}
  virtual int g() noexcept { return r; }
};
struct D : L, R {
  int d = 4;
  D() noexcept : V(5) {}
  int g() noexcept override { return d; }
};
struct E : D {
  long e = 6;
  E() noexcept : V(7) {}
};
int get() noexcept {
  D x;
  V *p = &x;
  R *r = &x;
  return p->f() + r->g() + x.v;
}
int get_e() noexcept {
  E y;
  V *p = &y;
  L *l = &y;
  return p->f() + l->f() + y.e;
}
