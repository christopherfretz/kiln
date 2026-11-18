// [expr.prim.lambda.capture]/3, the accepted side of
// capture-default-non-local.cpp: a capture-default in a block scope (also
// a lambda nested in a namespace-scope lambda's body, a local class's
// default member initializer), in a default member initializer (CWG1632:
// `this` is capturable there, also in a class template), and an
// init-capture alone at namespace scope (WP-1652).
//@ expect: compile-ok
//@ strict-diags
auto r1 = [n = 2] { return n; };
auto r2 = [] { return [&] { return 2; }(); };
auto r3 = [x = 1]() mutable { return [&] { return x; }(); };
struct S {
  int m = [=] { return 4; }();
  int k = [&] { return m; }();
  int j = [] { return [&] { return 1; }(); }();
};
template <class T> struct U { T t = [&] { return T(); }(); };
U<int> u;
int fn() {
  auto a = [&] { return 1; };
  static auto b = [=] { return 2; };
  struct L { int x = [=] { return 1; }(); };
  return a() + b() + L().x + r1() + r2() + r3();
}
