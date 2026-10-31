//@ expect: compile-ok
//@ strict-diags
// [dcl.struct.bind] in constant evaluation ([expr.const]): bindings to
// arrays, data members and tuple-like types (a member get, and a get found
// by argument-dependent lookup, with user std::tuple_size/tuple_element)
// are evaluated by the Oven, including reference forms that write through
// to the original object, range-for loops, and namespace scope bindings to
// a constexpr object.
namespace std {
template <class T> struct tuple_size;
template <unsigned long I, class T> struct tuple_element;
}
struct P { int x, y; };
struct Q {
  int a;
  long b;
  template <unsigned long I> constexpr auto get() const {
    if constexpr (I == 0) return a; else return b;
  }
};
template <> struct std::tuple_size<Q> { static constexpr unsigned long value = 2; };
template <> struct std::tuple_element<0, Q> { using type = int; };
template <> struct std::tuple_element<1, Q> { using type = long; };
struct R { int a, b; };
template <unsigned long I> constexpr int get(const R &r) { return I == 0 ? r.a : r.b; }
template <> struct std::tuple_size<R> { static constexpr unsigned long value = 2; };
template <unsigned long I> struct std::tuple_element<I, R> { using type = int; };
constexpr int f1() {
  P p{1, 2};
  auto [x, y] = p;
  auto &[m, n] = p;
  m = 10;
  return x + y + p.x + n;
}
constexpr int f2() {
  int a[3] = {1, 2, 3};
  auto [u, v, w] = a;
  auto &[r, s, t] = a;
  r = 5;
  return u + v + w + a[0] + s;
}
constexpr long f3() {
  Q q{3, 4};
  auto [i, j] = q;
  auto &&[k, l] = Q{5, 6};
  return i + j + k + l;
}
constexpr int f4() {
  R r{7, 8};
  auto [i, j] = r;
  const auto &[k, l] = r;
  return i + j + k + l;
}
constexpr int f5() {
  P ps[2] = {{1, 2}, {3, 4}};
  int s = 0;
  for (auto [x, y] : ps) s += x * y;
  for (auto &[x, y] : ps) x = y;
  return s + ps[0].x + ps[1].x;
}
template <class T> constexpr int f6(T t) {
  auto [p, q] = t;
  return p - q;
}
static_assert(f1() == 1 + 2 + 10 + 2);
static_assert(f2() == 6 + 5 + 2);
static_assert(f3() == 18);
static_assert(f4() == 30);
static_assert(f5() == 14 + 2 + 4);
static_assert(f6(P{9, 4}) == 5);
static_assert(f6(R{1, 4}) == -3);
constexpr P cp{1, 2};
constexpr int g() { auto [a, b] = cp; return a * 10 + b; }
static_assert(g() == 12);
auto &[c, d] = cp;
static_assert(c == 1 && d == 2);
constexpr int arr[2] = {5, 6};
auto &[e0, e1] = arr;
static_assert(e0 + e1 == 11);
