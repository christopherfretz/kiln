// [expr.prim.lambda.closure]/5 (P0847): a lambda with an explicit object
// parameter. `this auto` deduces the closure type (`_ZZ...ENH...clIS_EE`:
// `H` marks the explicit object parameter in the mangled name), recursion
// goes through `self`, the call operator takes the object as an ordinary
// first parameter, and each capture access re-derives the closure from
// that parameter (clang `EmitLValueForLambdaField`), loading a reference
// parameter once per access. An init-capture of a constant is stored
// member-wise: a closure with captures has no default constructor, so it
// is not POD (no `__const` copy).
int fact(int n) {
  auto f = [](this auto self, int k) -> int { return k <= 1 ? 1 : k * self(k - 1); };
  return f(n);
}
int twice(int a) {
  auto r = [a](this auto const& self, int k) { return a + k + a; };
  return r(4);
}
int by_value(int a) {
  auto r = [a](this auto self, int k) { return a + k; };
  return r(4);
}
int fwd() {
  auto l = [x = 3](this auto&& self) { return x; };
  return l();
}
int unused_self() {
  auto r = [](this auto const& self, int k) { return k + 1; };
  return r(4);
}
template <class Self> int named(Self&& self) { return self.v; }
struct B {
  int m;
  int get() {
    auto r = [this](this auto const& s) { return m; };
    return r();
  }
  int v;
  int xobj(this B const& self) { return self.m + self.v; }
};
int use_b() { B b{1, 2}; return b.get() + b.xobj() + named(b); }
