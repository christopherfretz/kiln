// WP-462: the this-adjusting thunk of a variadic virtual function forwards
// its own arguments, variadic ones included, with a `musttail` call (clang
// `EmitMustTailThunk`); the ordinary epilogue follows in an unreachable
// block. Covers scalar, coerced, indirect (sret) and void returns and a
// virtual-base (vcall offset) adjustment.
struct Big { long a, b, c, d; };
struct Pair { int a, b; };
struct A { virtual int f(int, ...); virtual Big big(int, ...); int a; };
struct B {
  virtual int f(int, ...);
  virtual Big big(int, ...);
  virtual Pair pair(const char *, ...);
  virtual void v(...);
  int b;
};
struct C : A, B {
  int f(int x, ...) override;
  Big big(int, ...) override;
  Pair pair(const char *, ...) override;
  void v(...) override;
};
int C::f(int x, ...) { return x; }
Big C::big(int x, ...) { return Big{x, 0, 0, 0}; }
Pair C::pair(const char *, ...) { return Pair{1, 2}; }
void C::v(...) {}
struct V { virtual long w(long, ...); long v; };
struct D : virtual V { long w(long x, ...) override; };
long D::w(long x, ...) { return x; }
D d;
