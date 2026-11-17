// Before C++20 no closure type has a default constructor (C++17
// [expr.prim.lambda.closure]): a captureless closure is not trivial
// and not POD either, though it stays trivially copyable (WP-1520).
//@ args: -std=c++17
//@ strict-diags
void f() {
  int x = 0;
  auto l = [] { return 1; };
  auto c = [x] { return x; };
  auto e = [=] { return 1; };
  static_assert(!__is_trivial(decltype(l)), "");
  static_assert(!__is_pod(decltype(l)), "");
  static_assert(!__is_trivially_constructible(decltype(l)), "");
  static_assert(__is_trivially_copyable(decltype(l)), "");
  static_assert(!__is_trivial(decltype(c)), "");
  static_assert(!__is_trivial(decltype(e)), "");
  (void)l; (void)c; (void)e;
}
