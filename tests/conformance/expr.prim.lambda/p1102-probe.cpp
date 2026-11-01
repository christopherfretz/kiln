// [expr.prim.lambda.general]/1 (C++23, P1102R2): the parameter list `()` of
// a lambda may be omitted before a lambda-specifier, noexcept, an attribute,
// a trailing return type or a requires-clause. WP-185 feature probe.
//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
constexpr int f() {
  int n = 1;
  auto a = [n] mutable { return ++n; };
  auto b = [] constexpr { return 2; };
  auto c = [] noexcept { return 3; };
  auto d = [] -> long { return 4; };
  return a() + b() + c() + int(d());
}
static_assert(f() == 11);
static_assert(noexcept([] noexcept {}()));
