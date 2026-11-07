// [class.mfct.non.static], [class.virtual], Itanium C++ ABI §2.6.4
// (thunks): member functions returning and taking small and large classes
// — `this` first then `sret` or the reverse as the ABI orders them,
// `this`-adjusting thunks and covariant-free virtual overrides in a
// secondary base, static members, and calls through pointers to member
// functions (spec/landed/abi/WP-766.md). Compared with clang 19 and
// clang 22 on both targets.
struct Small { int a, b; };
struct Big { long a[5]; };
struct Hfa { double a, b, c; };
struct E {};
struct NT { int a; ~NT(); };
struct A { virtual Small fa(); long pad; };
struct B {
  virtual Small fs(Small);
  virtual Big fb(Big, int);
  virtual Hfa fh(Hfa);
  virtual E fe(E);
  virtual NT fn(NT);
  int x;
};
struct C : A, B {
  Small fs(Small) override;
  Big fb(Big, int) override;
  Hfa fh(Hfa) override;
  E fe(E) override;
  NT fn(NT) override;
};
Small C::fs(Small s) { return s; }
Big C::fb(Big b, int) { return b; }
Hfa C::fh(Hfa h) { return h; }
E C::fe(E e) { return e; }
NT C::fn(NT n) { return NT{n.a}; }
struct M {
  Small s(Small) const;
  Big b(Big);
  static Big sb(Big);
  E e(E, int) &&;
  Hfa h() volatile;
};
long u_fs(B &b, Small s) { return b.fs(s).a; }
long u_fb(B &b, Big g) { return b.fb(g, 1).a[4]; }
double u_fh(B &b, Hfa h) { return b.fh(h).c; }
void u_fe(B &b) { b.fe(E{}); }
void u_fn(B &b, NT &n) { b.fn(n); }
long u_ms(M &m, Small s) { return m.s(s).b; }
long u_mb(M &m, Big g) { return m.b(g).a[0]; }
long u_sb(Big g) { return M::sb(g).a[1]; }
E u_me(M &&m, E e) { return static_cast<M &&>(m).e(e, 3); }
double u_mh(M &m) { return m.h().a; }
long u_pb(M &m, Big g) {
  Big (M::*pb)(Big) = &M::b;
  return (m.*pb)(g).a[2];
}
long u_ps(B &b, Small s) {
  Small (B::*ps)(Small) = &B::fs;
  return (b.*ps)(s).a;
}
long u_c(C &c, Small s) { return c.fs(s).a; }
