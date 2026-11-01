// [expr.const]/5.9: a mutable member of an object whose lifetime began
// within the evaluation may be read and modified: a local, the object being
// initialized (and its subobjects), a temporary. A lifetime-extended
// temporary usable in constant expressions still exposes its non-mutable
// members.
struct S {
  int k;
  mutable int m;
  constexpr S(int a) : k(a), m(a + 1) {}
  constexpr int get() const { return m; }
};
constexpr int f() {
  const S t{3};
  t.m = 7;
  return t.get();
}
static_assert(f() == 7);
struct W {
  S s;
  int v;
  constexpr W() : s(1), v(s.m) {}
};
constexpr W w;
static_assert(w.v == 2);
static_assert(w.s.k == 1);
constexpr int g(const S &s) { return s.m; }
static_assert(g(S(4)) == 5);
constexpr const S &r = S(5);
static_assert(r.k == 5);
struct C {
  mutable int n = 0;
  constexpr int bump() const { return ++n; }
};
constexpr int h() {
  const C c;
  c.bump();
  return c.bump();
}
static_assert(h() == 2);
