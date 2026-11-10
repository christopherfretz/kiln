// [expr.prim.lambda.capture]/8, /10, [basic.def.odr]/5: which locals a
// generic lambda's capture-default captures, decided in the template
// definition (clang's speculative capture of a potential odr-use; WP-581).
// A variable that might be usable in constant expressions is captured
// when its full-expression depends on a lambda parameter, or when it is
// not usable in constant expressions; a non-dependent lvalue-to-rvalue
// conversion of a constant variable, and an unevaluated operand, are no
// odr-uses. The closure's size shows the members.
//@ expect: compile-ok
//@ skip(gcc): pins clang's closure layout (which entities are captured by reference); a closure's size is unspecified ([expr.prim.lambda.closure]/2)
//@ std: c++20
void f(int r) {
  const int n = 10;
  const int m = r;
  const int& rr = r;
  auto a = [=](auto x) { return x + n; };
  static_assert(sizeof(a) == sizeof(int));
  auto b = [=](auto x) { int y = n; return x + y; };
  static_assert(sizeof(b) == 1);
  auto c = [&](auto x) { return x + rr; };
  static_assert(sizeof(c) == sizeof(int*));
  auto d = [&](auto x) { int y = rr; return x + y; };
  static_assert(sizeof(d) == sizeof(int*));
  auto e = [=](auto x) { int y = m; return x + y; };
  static_assert(sizeof(e) == sizeof(int));
  auto g = [=](auto x) { return x + sizeof(rr); };
  static_assert(sizeof(g) == 1);
  auto h = [&](auto x) { decltype(rr) k = x; return k; };
  static_assert(sizeof(h) == 1);
  auto i = [&](auto x) { return [&](auto y) { return x + y + rr; }; };
  static_assert(sizeof(i) == sizeof(int*));
}
