// [expr.new]/22, /24, [expr.delete]/6, [expr.const]/5.20: new-expressions
// of class type construct their objects (elements in order), delete
// destroys them (array elements in reverse order) before deallocating;
// copy construction, default member initializers, parenthesized aggregate
// initialization and default construction of arrays in constant
// evaluation.
struct Log { int v[16] = {}; int n = 0; constexpr void add(int x) { v[n++] = x; } };
struct T {
  Log *l; int id;
  constexpr T(Log *l, int id) : l(l), id(id) { l->add(id); }
  constexpr ~T() { l->add(-id); }
};
constexpr bool heap() {
  Log l;
  T *p = new T(&l, 8);
  delete p;
  T *q = new T[3]{{&l, 1}, {&l, 2}, {&l, 3}};
  delete[] q;
  int e[] = {8, -8, 1, 2, 3, -3, -2, -1};
  for (int i = 0; i < 8; ++i)
    if (l.v[i] != e[i]) return false;
  return l.n == 8;
}
static_assert(heap());
struct Owner {
  int *p;
  constexpr Owner(int v) : p(new int(v)) {}
  constexpr Owner(const Owner &o) : p(new int(*o.p + 1)) {}
  constexpr ~Owner() { delete p; }
};
constexpr int owners() {
  Owner a(1);
  Owner b = a;
  Owner c(b);
  Owner *h = new Owner(c);
  int r = *h->p;
  delete h;
  return r * 10 + *a.p;
}
static_assert(owners() == 41);
struct Def { int x = 4; int y; constexpr Def() : y(x + 1) {} };
static_assert(Def().y == 5);
struct NT { int x; constexpr NT() : x(1) {} };
constexpr int array_default() { NT a[4]; NT *h = new NT[5]; int r = a[0].x + a[3].x + h[4].x; delete[] h; return r; }
static_assert(array_default() == 3);
struct PL { int a; int b; };
constexpr int paren_list() { PL p(1, 2); PL *h = new PL(3, 4); int r = p.a + p.b + h->b; delete h; return r; }
static_assert(paren_list() == 7);
