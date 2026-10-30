// [class.temporary]/4, /6, [expr.const]: a temporary is destroyed at the
// end of its full-expression, in reverse order of construction; a temporary
// bound to a reference lives as long as the reference; an aggregate's
// member temporaries are destroyed with the aggregate.
struct Log { int v[16] = {}; int n = 0; constexpr void add(int x) { v[n++] = x; } };
struct T {
  Log *l; int id;
  constexpr T(Log *l, int id) : l(l), id(id) { l->add(id); }
  constexpr ~T() { l->add(-id); }
};
constexpr int full_expression() {
  Log l;
  int r = T(&l, 5).id + T(&l, 6).id;
  return r * 100 + l.n * 10 + (l.v[2] == -6 && l.v[3] == -5);
}
static_assert(full_expression() == 1141);
constexpr bool extended() {
  Log l;
  { const T &r = T(&l, 7); T &&s = T(&l, 8); l.add(r.id + s.id); }
  int e[] = {7, 8, 15, -8, -7};
  for (int i = 0; i < 5; ++i)
    if (l.v[i] != e[i]) return false;
  return true;
}
static_assert(extended());
struct Agg { int a; T t; };
constexpr bool aggregate() {
  Log l;
  { Agg g{1, T(&l, 9)}; l.add(g.a); }
  return l.n == 3 && l.v[0] == 9 && l.v[1] == 1 && l.v[2] == -9;
}
static_assert(aggregate());
struct S { int a, b; constexpr int sum() const { return a + b; } };
constexpr int sum(const S &s) { return s.sum(); }
static_assert(sum(S{3, 4}) == 7);
static_assert(S{1, 2}.sum() == 3);
