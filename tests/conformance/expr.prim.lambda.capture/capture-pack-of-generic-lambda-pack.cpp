// [expr.prim.lambda.capture]/17, [temp.variadic]/5: a simple-capture
// pack or init-capture pack in a lambda nested in a generic lambda names
// a pack that is still unexpanded when the enclosing function template is
// instantiated (the generic lambda's own parameter pack); it is expanded
// when the generic lambda's call operator is specialized, as are packs
// from both levels in one capture list.
//@ std: c++20
//@ expect: compile-ok
//@ strict-diags
template <class... T> constexpr auto f(T... t) {
  return [=](auto... u) {
    auto a = [u...] { return (u + ... + 0); }();
    auto b = [...v = u] { return (v + ... + 0); }();
    auto c = [&...w = u] { return (w + ... + 0); }();
    auto d = [t..., u...] { return (t + ... + 0) * (u + ... + 0); }();
    return a + b + c + d;
  };
}
template <class... T> constexpr auto g(T...) {
  return [](auto... u) {
    return [u...]<class... X>(X... x) { return [u..., x...] { return (u + ... + 0) * 10 + (x + ... + 0); }(); };
  };
}
static_assert(f(1, 2)(3, 4) == 7 + 7 + 7 + 21);
static_assert(g(1)(1, 2)(3, 4, 5) == 42);
