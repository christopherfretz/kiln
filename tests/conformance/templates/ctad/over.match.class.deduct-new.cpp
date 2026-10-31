// [expr.new]/2, [dcl.type.class.deduct]/2: `new C(args)` deduces the
// allocated type.
//@ strict-diags
template<class T, class U> struct P {
  T a;
  U b;
  constexpr P(T x, U y) : a(x), b(y) {}
};
template<class T = int> struct Def {};
auto *p = new P(1, 2.0f);
static_assert(__is_same(decltype(p), P<int, float> *));
auto *q = new P{'a', 2L};
static_assert(__is_same(decltype(q), P<char, long> *));
auto *d = new Def;
static_assert(__is_same(decltype(d), Def<int> *));
constexpr int f() {
  auto *r = new P(40, 2);
  int s = r->a + r->b;
  delete r;
  return s;
}
static_assert(f() == 42);
