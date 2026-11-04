// [expr.prim.lambda.capture]/7, [basic.def.odr]/5: a constexpr local of an
// enclosing function named as a template argument inside a lambda with no
// capture-default is not odr-used (an lvalue-to-rvalue conversion is
// applied to a variable usable in constant expressions), so it needs no
// capture. kiln decided the potential odr-uses of every pending name when
// the first template argument's converted constant expression completed,
// so `k` as a later argument (`f<0, k>()`, `W<0, k>`) was taken as an
// odr-use: "cannot be implicitly captured", or for a function template an
// "invalid explicitly-specified argument". A binding generator's
// `[]() consteval { return f<"verb", getter, Carrier>(); }()` hit it.
//@ expect: run
//@ exit: 0
template <int V, int M> constexpr int f() { return M; }
template <int V, int M> struct W { static constexpr int v = M; };
template <int V, int M> constexpr int vt = M;
int g() {
  constexpr int k = 2;
  int a = [] { return f<0, k>(); }();
  int b = [] { return W<0, k>::v; }();
  int c = [] { return vt<0, k>; }();
  int d = [] { return f<k, k + 1>() + f<1, k>(); }();
  return a + b + c + d;
}
consteval int h() {
  constexpr int k = 3;
  return [] consteval { return f<1, k>(); }();
}
static_assert(h() == 3);
int main() { return g() == 2 + 2 + 2 + 5 ? 0 : 1; }
