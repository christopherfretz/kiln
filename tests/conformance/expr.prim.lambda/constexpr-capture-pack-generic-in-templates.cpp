// [expr.prim.lambda.capture]/17, [temp.variadic]/5: a capture pack of a
// lambda nested in a generic lambda names the generic lambda's parameter
// pack; instantiating the enclosing function template keeps it a capture
// pack (its pack belongs to the generic lambda), and the generic lambda's
// instantiation expands it.
//@ expect: compile-ok
constexpr int g(auto... xs) { return (xs + ... + 0); }
template <class... T> constexpr int f(T... ts) {
  auto l = [=](auto... us) { return [us..., ts...] { return g(us...) * 10 + g(ts...); }(); };
  return l(1, 2);
}
static_assert(f(5, 6) == 41);
template <class... T> constexpr int h(T... ts) {
  auto l = [](auto... us) { return [&us...] { return g(us...); }(); };
  return l(ts...);
}
static_assert(h(5, 6) == 11);
template <class... T> constexpr int k(T... ts) {
  auto l = [](auto... us) { return [...vs = us] { return g(vs...); }(); };
  return l(ts...) + [](auto... us) { return g([us] { return us; }()...); }(ts...);
}
static_assert(k(5, 6) == 22);
template <class... T> constexpr int z(T... ts) {
  return [](auto... us) { return g(sizeof(us)...); }(ts...);
}
static_assert(z('a', 1) == 5);
