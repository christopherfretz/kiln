// WP-665 ([dcl.init.aggr], [basic.start.static]): a constant table whose
// elements hold pointers, data and function member pointers and unions
// (nested ones too) with a pointer or a member pointer active. Codegen looks each field's
// relocation (and each union's active member) up in the whole table's
// byte image by offset; a lookup that returns the wrong entry puts the
// wrong address, a null, or the wrong member into some element, and the
// IR no longer matches clang's. `table` names its union members in the
// initializer lists; `grid` is built by a constexpr function, so its
// unions' active members are the ones the evaluation recorded.
struct Node { int v; };
Node na{1}, nb{2}, nc{3};
int f0(int x) { return x; }
int f1(int x) { return x + 1; }
struct K {
  int a, b;
  int get() const { return a; }
  int put(int x) { return b = x; }
};
struct SV { const char *p; unsigned long n; };
union U {
  const Node *node;
  int K::*mp;
  long tag;
};
// Nested unions at one offset: both active members are recorded there.
union V {
  U inner;
  const char *s;
};
struct Row {
  SV name;
  const Node *node;
  int (*fn)(int);
  int K::*field;
  int (K::*method)() const;
  U u;
  V v;
  const Row *next;
  unsigned long id;
};
static constexpr const char *names[] = {"zero", "one", "two", "three"};
constexpr Row table[6] = {
    {{names[0], 4}, &na, f0, &K::a, &K::get, {&nb}, {.inner = {.mp = &K::a}}, &table[1], 0},
    {{names[1], 3}, nullptr, f1, &K::b, nullptr, {.mp = &K::b}, {.s = "v1"}, &table[2], 1},
    {{names[2], 3}, &nb, nullptr, nullptr, &K::get, {.tag = 7}, {.inner = {&na}}, &table[0], 2},
    {{names[3], 5}, &nc, f0, &K::a, &K::get, {.mp = nullptr}, {}, nullptr, 3},
    {{nullptr, 0}, &na, f1, nullptr, nullptr, {&nc}, {.inner = {.tag = 9}}, &table[3], 4},
    {{names[1], 3}, &nc, f1, &K::b, &K::get, {.node = nullptr}, {.s = nullptr}, &table[4], 5},
};
const Row *row(unsigned i) { return &table[i]; }
int call(unsigned i, K &k) { return (k.*table[i].method)() + k.*table[i].field; }

struct Cell {
  U u;
  V v;
  const Node *p;
};
struct Grid {
  Cell c[8];
};
constexpr Cell cell(int i) {
  Cell c{};
  switch (i % 4) {
  case 0: c.u.mp = &K::b; c.v.s = names[i % 4]; break;
  case 1: c.u.node = &nc; c.v.inner.mp = &K::b; break;
  case 2: c.u.tag = i; c.v.inner.node = &nb; break;
  default: c.u.mp = nullptr; c.v.inner.tag = -i; break;
  }
  c.p = i % 3 ? &na : nullptr;
  return c;
}
constexpr Grid make_grid() {
  Grid g{};
  for (int i = 0; i < 8; ++i)
    g.c[i] = cell(i);
  return g;
}
constexpr Grid grid = make_grid();
const Grid *get_grid() { return &grid; }
