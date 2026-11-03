// [expr.prim.lambda.capture]/6: init-captures with each initializer form --
// `= e`, `(e)`, `{e}`, `= {e, e}` (std::initializer_list) and the
// reference form `&r(e)` -- plus a class-type capture whose explicit copy
// constructor only direct-initialization may call, and an init-capture
// pack with a parenthesized initializer.
namespace std {
template <class E> class initializer_list {
  const E *b;
  decltype(sizeof 0) n;
  constexpr initializer_list(const E *p, decltype(sizeof 0) c) : b(p), n(c) {}
public:
  constexpr initializer_list() : b(nullptr), n(0) {}
  constexpr decltype(sizeof 0) size() const { return n; }
  constexpr const E *begin() const { return b; }
};
}
struct X {
  int v;
  X(int a) : v(a) {}
  explicit X(const X &o) : v(o.v + 1) {}
};
int forms(int a) {
  auto l = [x{1}, y(2), z = a, &r(a)] { return x + y + z + r; };
  return l();
}
int list(int a) {
  auto l = [w = {a, 2, 3}] { return (int)w.size() + *w.begin(); };
  return l();
}
int expl(X &o) {
  auto l = [q(o), p{o}] { return q.v + p.v; };
  return l();
}
int mut(int a) {
  auto l = [c{a}]() mutable { return ++c; };
  l();
  return l();
}
template <class... T> int pack(T... ts) {
  return [...xs(ts), &...rs{ts}] { return (xs + ... + 0) + (rs + ... + 0); }();
}
int use_pack() { return pack(1, 2, 3); }
