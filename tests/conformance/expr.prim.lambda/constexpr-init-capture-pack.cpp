// [expr.prim.lambda.capture]/17, [temp.variadic]/5: an init-capture pack
// `...xs = init` declares a pack of captures, one per element of the
// expanded initializer pattern; `&...xs = init` captures by reference.
// The pack is expanded when the enclosing template (or generic lambda) is
// instantiated, or kept when that instantiation leaves the pattern's
// packs unexpanded (a generic lambda in a function template).
//@ expect: compile-ok
template <class T> constexpr T&& mv(T& t) { return static_cast<T&&>(t); }
struct M { int v; constexpr M(int x) : v(x) {} constexpr M(M&& o) : v(o.v) { o.v = -1; } };
template <class... T> constexpr int count(T... ts) {
  return [...xs = ts] { return sizeof...(xs); }();
}
static_assert(count() == 0 && count(1, 'a', 2.0) == 3);
constexpr int generic() {
  auto g = [](auto... us) { return [...xs = us] { return (xs * ... * 1); }(); };
  return g(2, 3, 4);
}
static_assert(generic() == 24);
template <class... T> constexpr int keep(T... ts) {
  auto g = [=](auto... us) { return [...xs = us, ...ys = ts] { return (xs + ... + 0) * 10 + (ys + ... + 0); }(); };
  return g(1, 2);
}
static_assert(keep(4, 5) == 39);
template <class... T> constexpr int byref(T... ts) {
  [&...xs = ts] { ((xs += 1), ...); }();
  return (ts + ... + 0);
}
static_assert(byref(1, 2, 3) == 9);
template <class... T> constexpr int moved(T... ts) {
  int r = [...xs = mv(ts)] { return (xs.v + ... + 0); }();
  return r + (ts.v + ... + 0);
}
static_assert(moved(M(1), M(2)) == 3 - 2);
template <class... T> constexpr int sum(T... ts) {
  return [...xs = mv(ts)] { return (xs + ... + 0); }();
}
static_assert(sum() == 0 && sum(1, 2, 3) == 6);
