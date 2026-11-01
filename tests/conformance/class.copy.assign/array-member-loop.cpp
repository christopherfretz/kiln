// [class.copy.assign]/12.2: an implicit copy/move assignment assigns each
// element of an array member (a loop for a non-trivial element assignment,
// `memcpy` for a trivially copyable one), at compile time and at run time.
//@ expect: run
//@ stdout: 1 2 3 4 5 6 7 8 c
//@ stdout: 10 20 30 40 m
//@ stdout: 3.3 6
extern "C" int printf(const char *, ...);
struct E {
  int v = 0;
  char k = '-';
  constexpr E() = default;
  constexpr E(int x) : v(x) {}
  constexpr E& operator=(const E& o) { v = o.v; k = 'c'; return *this; }
  constexpr E& operator=(E&& o) { v = o.v; k = 'm'; return *this; }
};
struct S { E a[2]; E b[2][3]; };
struct M { E a[4]; };
struct D { double d[3]; int n[2][1]; };
constexpr S make() { return S{{1, 2}, {{3, 4, 5}, {6, 7, 8}}}; }
constexpr int copied() {
  S x, y = make();
  x = y;
  return x.a[1].v * 100 + x.b[1][2].v * 10 + (x.b[0][0].k == 'c');
}
static_assert(copied() == 281);
constexpr int moved() {
  M x, y{{10, 20, 30, 40}};
  x = static_cast<M&&>(y);
  return x.a[3].v + (x.a[0].k == 'm');
}
static_assert(moved() == 41);
constexpr bool trivial() {
  D a{{1.1, 2.2, 3.3}, {{4}, {6}}}, c{};
  const D& r = a;
  c = r;
  double x[3] = {1, 2, 3}, y[3] = {};
  __builtin_memcpy(&y, &x, sizeof x);
  return c.d[2] == 3.3 && c.n[1][0] == 6 && y[2] == 3;
}
static_assert(trivial());
int main() {
  S x, y = make();
  x = y;
  for (auto& e : x.a) printf("%d ", e.v);
  for (auto& r : x.b) for (auto& e : r) printf("%d ", e.v);
  printf("%c\n", x.b[1][2].k);
  M m, n{{10, 20, 30, 40}};
  m = static_cast<M&&>(n);
  for (auto& e : m.a) printf("%d ", e.v);
  printf("%c\n", m.a[0].k);
  D a{{1.1, 2.2, 3.3}, {{4}, {6}}}, c{};
  c = a;
  printf("%.1f %d\n", c.d[2], c.n[1][0]);
}
