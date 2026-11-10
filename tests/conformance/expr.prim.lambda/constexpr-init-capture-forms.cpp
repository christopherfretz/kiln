// [expr.prim.lambda.capture]/6 in constant evaluation: each init-capture
// form, in a function and in templates (the initializer is instantiated
// like any other; the pack forms `[...xs(ts)]`, `[...ys{ts}]`, `[&...rs(ts)]`
// included), and an explicit copy constructor selected by
// direct-initialization.
//@ expect: compile-ok
//@ xfail(gcc): GCC 16.1 rejects a direct-initialized init-capture `[q(x)]`/`[q{x}]` of a class with an explicit copy constructor ([expr.prim.lambda.capture]/6)
#include <initializer_list>
struct X {
  int v;
  constexpr X(int a) : v(a) {}
  explicit constexpr X(const X &o) : v(o.v + 100) {}
};
constexpr int f() {
  int a = 3;
  auto l = [x{1}, y(2), z = a, &r(a), w = {4, 5}] { return x + y + z + r + (int)w.size(); };
  X ox(7);
  auto m = [q(ox)] { return q.v; };
  auto mut = [c{0}]() mutable { return ++c; };
  mut();
  return l() + m() * 100 + mut() * 100000;
}
static_assert(f() == 210711);
template <class... Ts> constexpr int g(Ts... ts) {
  auto l = [...xs(ts), ...ys{ts}, &...rs(ts)] {
    return (0 + ... + xs) + (0 + ... + ys) + (0 + ... + rs);
  };
  return l();
}
static_assert(g(1, 2, 3) == 18);
static_assert(g() == 0);
template <class T> constexpr int h(T t) {
  auto l = [x(t), y{t}, z = {t, t}, &r(t)] { return x + y + (int)z.size() + r; };
  return l();
}
static_assert(h(5) == 17);
template <class T> constexpr int k(T t) {
  auto l = [x(t)] { return x.v; };
  return l();
}
static_assert(k(X(1)) == 101);
