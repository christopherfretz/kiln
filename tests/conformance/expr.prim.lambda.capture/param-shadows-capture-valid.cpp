// [expr.prim.lambda.capture]/5 applies to explicit captures only: a
// parameter may have the name of an entity captured implicitly by `[=]` or
// `[&]`, of an enclosing local that is not captured, or of an explicit
// capture of another lambda; unnamed parameters are fine. Invalid pair:
// param-shadows-capture-forms.cpp (WP-1015).
//@ std: c++20
//@ expect: compile-ok
int f() {
  int x = 1, y = 2;
  auto a = [=](int x) { return x + y; };
  auto b = [&](int y) { return x + y; };
  auto c = [x](int) { return x; };
  auto d = [y](int x) { return [x](int y) { return x + y; }(y); };
  auto e = [](int x) { return x; };
  return a(1) + b(2) + c(3) + d(4) + e(5);
}
template<class... Ts> int g(Ts... ts) {
  return [ts...](int n) { return (n + ... + ts); }(0);
}
int k = g(1, 2);
