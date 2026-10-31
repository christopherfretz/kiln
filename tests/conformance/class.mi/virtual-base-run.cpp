// [class.mi]/4-6, [class.base.init]/7, [class.cdtor]/4, [conv.ptr]/3:
// virtual bases at run time (WP-080): one shared subobject in a
// diamond, initialized by the most derived class only, conversions to it
// through the vbase offset in the vtable, null conversions, and virtual
// dispatch while a base subobject is constructed (construction vtables).
//@ expect: run
//@ stdout: ctor L:10 R:20
//@ stdout: D 7 4 5 shared 1
//@ stdout: E 9 9 6 7
//@ stdout: null 1
extern "C" int printf(const char *, ...);
struct V {
  int v;
  V(int x) noexcept : v(x) {}
  virtual int f() noexcept { return v; }
};
struct L : virtual V {
  int l = 2;
  L() noexcept : V(10) { seen = f(); }
  int seen;
  int f() noexcept override { return l + v; }
};
struct R : virtual V {
  int r = 3;
  R() noexcept : V(20) { seen = f(); }
  int seen;
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
int main() {
  L lone;
  R rone;
  printf("ctor L:%d R:%d\n", lone.f() - lone.l, rone.f());
  D x;
  V *p = &x;
  R *r = &x;
  V *pl = static_cast<L *>(&x);
  V *pr = static_cast<R *>(&x);
  printf("D %d %d %d shared %d\n", p->f(), r->g(), x.v, pl == pr);
  E y;
  V *q = &y;
  L *l = &y;
  printf("E %d %d %ld %d\n", q->f(), l->f(), y.e, y.v);
  R *nr = nullptr;
  V *nv = nr;
  printf("null %d\n", nv == nullptr);
  return 0;
}
