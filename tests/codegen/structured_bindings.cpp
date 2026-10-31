// Structured bindings ([dcl.struct.bind]): the holding variable `e` is an
// ordinary (unnamed) variable, and each binding is an lvalue naming `e[i]`
// or `e.m`, or (tuple-like) a reference variable initialized from get<i>.
// Namespace scope and static holding variables mangle as
// `DC <source-name>+ E` (never L-prefixed, even in extern "C").
namespace std {
template <class T> struct tuple_size;
template <unsigned long I, class T> struct tuple_element;
}
struct P { int x, y; };
struct Q {
  int a;
  float b;
  template <unsigned long I> auto get() const {
    if constexpr (I == 0) return a; else return b;
  }
};
template <> struct std::tuple_size<Q> { static constexpr unsigned long value = 2; };
template <> struct std::tuple_element<0, Q> { using type = int; };
template <> struct std::tuple_element<1, Q> { using type = float; };
struct BF { int a : 3; int b; };

int arr(int *p) {
  int a[2] = {p[0], p[1]};
  auto [u, v] = a;
  auto &[w, z] = a;
  return u + v + w + z;
}
int cls(P p) {
  auto [x, y] = p;
  auto &[m, n] = p;
  m = 3;
  return x + y + m + n;
}
int tup(Q q) {
  auto [i, f] = q;
  auto &&[j, g] = Q{1, 2};
  return i + (int)f + j + (int)g;
}
int lam(P p) {
  auto [x, y] = p;
  auto l = [&] { return x + y; };
  auto c = [x] { return x; };
  return l() + c();
}
int bitfield(BF bf) {
  auto &[x, y] = bf;
  const int &c = x;
  x = 2;
  return c + x + y;
}
int loop(P (&ps)[2]) {
  int s = 0;
  for (auto [x, y] : ps) s += x * y;
  return s;
}

static auto [a, b] = P{1, 2};
namespace N { auto [c, d] = P{1, 2}; }
namespace { auto [e, f] = P{1, 2}; }
extern "C" { auto [g, h] = P{1, 2}; }
constexpr P cp{1, 2};
auto [gx, gy] = cp;
int use() { return a + N::c + e + g + gx + gy; }
int sl() {
  static auto [s1, s2] = P{5, 6};
  return s1 + s2;
}
