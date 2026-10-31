// [over.ref], [temp.deduct.call]/6: an `operator->` member template is a
// candidate whose arguments come from default template arguments; the
// chain continues through the specialization's result.
//@ expect: compile-ok
struct P { int x = 3; };
struct S {
  P p;
  template <class T = int> constexpr P* operator->() { return &p; }
};
constexpr int f() { S s; return s->x; }
static_assert(f() == 3);
template <class T> struct W {
  T t;
  template <class U = T> constexpr U* operator->() { return &t; }
};
constexpr int k() { W<P> w; return w->x; }
static_assert(k() == 3);
struct M {
  P p;
  template <class T = int> constexpr P* operator->() { return &p; }
  template <class T = int> constexpr const P* operator->() const { return &p; }
};
constexpr int m() { M a; const M b; return a->x + b->x; }
static_assert(m() == 6);
struct Chain {
  W<P> w;
  template <class T = W<P>> constexpr T& operator->() { return w; }
};
constexpr int c() { Chain ch; return ch->x; }
static_assert(c() == 3);
