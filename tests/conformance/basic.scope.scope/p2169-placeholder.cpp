// [basic.scope.scope]/5 (C++26, P2169R4): a declaration of `_` that is a
// variable of automatic storage duration, a structured binding not at
// namespace scope, an init-capture or a non-static data member is
// name-independent: it conflicts with no earlier declaration of its scope,
// and `_` stays usable while it is declared once.
//@ expect: run
//@ std: c++26
//@ strict-diags
struct P { int a, b; };
struct S {
  int _ = 1;
  int _ = 2;
  int x = 3;
};
static_assert(sizeof(S) == 3 * sizeof(int));

constexpr int locals() {
  int _ = 1;
  int _ = 2;
  auto [_, b] = P{3, 4};
  auto [a, _] = P{5, 6};
  int r = a + b;
  {
    int _ = 7; // A new scope: the only `_` there.
    r += _;
  }
  for (int _ = 0, _ = 1; r < 0;) {}
  return r;
}
static_assert(locals() == 16);

constexpr int captures() {
  int v = 10;
  auto l = [_ = v, _ = 2, w = v] { return w; };
  auto m = [_ = 1] { return _ + 1; }; // One init-capture `_`: usable.
  return l() + m();
}
static_assert(captures() == 12);

template <class T> constexpr T tmpl(T t) {
  T _ = t;
  T _ = t + 1;
  auto [_, y] = P{1, 2};
  auto g = [_ = t](auto u) { return u; };
  return y + g(t);
}
static_assert(tmpl(5) == 7);

constexpr int generic = [](auto t) {
  auto _ = t;
  auto _ = t * 2;
  return t;
}(4);
static_assert(generic == 4);

int main() {
  S s;
  if (s.x != 3 || locals() != 16 || captures() != 12 || tmpl(1L) != 3) return 1;
  int _ = 1;
  return _ - 1;
}
