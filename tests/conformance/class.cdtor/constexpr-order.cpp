// [class.base.init]/13, [class.dtor]/13, [expr.const]: in constant
// evaluation, members are constructed in declaration order after the base
// classes and destroyed in the reverse order, after the destructor's body;
// arrays of class type are destroyed from the last element.
struct Log { int v[16] = {}; int n = 0; constexpr void add(int x) { v[n++] = x; } };
struct T {
  Log *l; int id;
  constexpr T(Log *l, int id) : l(l), id(id) { l->add(id); }
  constexpr ~T() { l->add(-id); }
};
struct Pair { T a; T b; constexpr Pair(Log *l) : b(l, 2), a(l, 1) {} };
constexpr bool members() {
  Log l;
  { Pair p(&l); }
  int e[] = {1, 2, -2, -1};
  for (int i = 0; i < 4; ++i)
    if (l.v[i] != e[i]) return false;
  return l.n == 4;
}
static_assert(members());
struct Base { Log *l; constexpr Base(Log *l) : l(l) { l->add(10); } constexpr ~Base() { l->add(-10); } };
struct Der : Base { T t; constexpr Der(Log *l) : Base(l), t(l, 3) { l->add(4); } constexpr ~Der() { l->add(-4); } };
constexpr bool bases() {
  Log l;
  { Der d(&l); }
  int e[] = {10, 3, 4, -4, -3, -10};
  for (int i = 0; i < 6; ++i)
    if (l.v[i] != e[i]) return false;
  return l.n == 6;
}
static_assert(bases());
constexpr bool arrays() {
  Log l;
  { T a[3] = {{&l, 1}, {&l, 2}, {&l, 3}}; }
  int e[] = {1, 2, 3, -3, -2, -1};
  for (int i = 0; i < 6; ++i)
    if (l.v[i] != e[i]) return false;
  return true;
}
static_assert(arrays());
constexpr bool scopes() {
  Log l;
  for (int i = 1; i <= 2; ++i) { T t(&l, i); if (i == 2) break; }
  int e[] = {1, -1, 2, -2};
  for (int i = 0; i < 4; ++i)
    if (l.v[i] != e[i]) return false;
  return l.n == 4;
}
static_assert(scopes());
constexpr int early(Log &l, bool b) { T t(&l, 5); if (b) return l.n; T u(&l, 6); return l.n; }
constexpr bool returns() {
  Log l;
  int a = early(l, true);
  int b = early(l, false);
  int e[] = {5, -5, 5, 6, -6, -5};
  for (int i = 0; i < 6; ++i)
    if (l.v[i] != e[i]) return false;
  return a == 1 && b == 4;
}
static_assert(returns());
