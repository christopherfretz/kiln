// This-adjusting thunks forwarding scalar, reference and short args and
// an sret aggregate return. (noexcept: no EH before WP-100.)
struct Big { long x[5]; };
struct A { int a = 1; virtual void fa() noexcept {} };
struct B {
  int b = 2;
  virtual int h(int x, const A& r, short s) noexcept { return x; }
  virtual Big big(long k) noexcept { return Big{{k}}; }
};
struct C : A, B {
  int h(int x, const A& r, short s) noexcept override { return x + s; }
  Big big(long k) noexcept override { return Big{{k, k}}; }
};
int use(B* b, const A& a) noexcept { return b->h(1, a, 2) + (int)b->big(3).x[0]; }
int mk() noexcept { C c; return use(&c, c); }
