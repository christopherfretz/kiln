// The primary interface of module-bodies-image.cpp and
// module-bodies-replay.cpp: attributes on its module declaration, an
// `export import` of its partition, and inline and template definitions
// whose bodies declare locals, lambdas and block-scope classes.
export module extents_body [[]];
export import :part;
export inline int body(int a) {
  int x = a, *p = &x;
  auto twice = [x](int q) { int z = q; return z + x; };
  struct Local {
    int v;
    int get() const { return v; }
  };
  enum Kind { one, two };
  if (int c = a; c > 0)
    x += c;
  switch (int k = a + 1) {
  default:
    break;
  }
  return twice(1) + Local{*p}.get() + two;
}
export template <class T> T pattern(T v) {
  T copy = v;
  struct Holder {
    T held;
  } h{copy};
  auto get = [&h](int) { return h.held; };
  return get(0);
}
export inline int initialized = [] {
  int seed = 3;
  return seed;
}();
