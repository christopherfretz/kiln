// [class.virtual]/2 (final overriders), [class.mi]/4, [class.cdtor]/4 (a
// virtual call during construction uses the constructor's class), C++26
// P3533R2 (virtual bases in constant evaluation), spec/oven.md "Dynamic
// types": the Oven's virtual-call caches (WP-414). Overriders that live
// in virtual bases are reached through a complete object (no object-model
// walk) and, during a base-object constructor, through a relative word
// whose resolution is cached per position of the running class in its
// most derived object. Each shape is called many times, at several
// positions and in several enclosing objects; the values pin that each
// cached call reaches the right subobject.
//@ skip(clang): neither clang 19 nor clang 22 implements P3533
//@ expect: compile-ok
//@ strict-diags
//@ oven: steps<=140000 calls<=60000 peak_bytes<=768
struct V {
  int v = 1;
  constexpr virtual int f() const { return v; }
  constexpr virtual int g() const { return 0; }
  constexpr virtual ~V() = default;
};
struct W : virtual V {
  int w = 2;
  constexpr int f() const override { return w; }
};
struct X : virtual V {
  int x = 3;
  constexpr int g() const override { return x; }
};
struct D : virtual W, virtual X {
  int d = 4;
};
struct E : D {
  int e = 5;
  constexpr int g() const override { return e; }
};

// Complete objects: f is W's, g is X's (D) or E's.
constexpr long complete(int n) {
  D d[4];
  E e[4];
  const V *vs[8];
  const X *xs[8];
  for (int i = 0; i < 4; ++i) {
    vs[2 * i] = &d[i];
    vs[2 * i + 1] = &e[i];
    xs[2 * i] = &d[i];
    xs[2 * i + 1] = &e[i];
  }
  long sum = 0;
  for (int i = 0; i < n; ++i)
    sum += vs[i & 7]->f() * 10 + vs[i & 7]->g() + xs[i & 7]->f() * 100 + xs[i & 7]->g() * 1000;
  return sum;
}
// Per pair (D, E): (20 + 3 + 200 + 3000) + (20 + 5 + 200 + 5000).
static_assert(complete(8000) == 4000 * (3223 + 5225));

// Calls during base-object constructors: L<N>'s virtual base V is the
// most derived object's, at a position that differs per L<N> and per
// enclosing object.
template <int N> struct L : virtual V {
  int s = 0;
  constexpr L() {
    for (int i = 0; i < 50; ++i)
      s += f() + g() * 7;
  }
};
struct M : L<0>, L<1> {
  constexpr int g() const override { return 3; }
};
struct Pad {
  long p[3] = {};
};
struct Holder : Pad {
  int h = 9;
  M m;
};
constexpr long relative() {
  long sum = 0;
  for (int i = 0; i < 20; ++i) {
    M m;
    Holder hs[3];
    L<1> alone;
    // In the constructors f and g are V's (1 + 0 * 7, 50 times); after
    // construction g is M's.
    const L<1> &l1 = m;
    sum += m.L<0>::s + m.L<1>::s + alone.s + m.g() + l1.g();
    for (const Holder &h : hs)
      sum += h.m.L<0>::s + h.m.L<1>::s + h.m.f();
  }
  return sum;
}
static_assert(relative() == 20 * (50 + 50 + 50 + 3 + 3 + 3 * (50 + 50 + 1)));

// P's constructor calls W::f through its virtual base W, which Q places
// elsewhere than a complete P does.
struct P : virtual W {
  int got = 0;
  constexpr P() {
    for (int i = 0; i < 50; ++i)
      got += f();
  }
};
struct Q : Pad, P {
  int q = 6;
};
constexpr long nonrelative() {
  long sum = 0;
  for (int i = 0; i < 20; ++i) {
    P p;
    Q q;
    sum += p.got + q.got + q.f() + p.f();
  }
  return sum;
}
static_assert(nonrelative() == 20 * (100 + 100 + 2 + 2));
