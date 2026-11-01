// [basic.scope.scope]/5 (C++26, P2169R4): a variable named `_` is a
// name-independent declaration; several may appear in one scope, and the
// name is unusable once redeclared. WP-185 feature probe.
//@ expect: compile-ok
//@ strict-diags
//@ xfail(kiln): placeholder variables not implemented ("redefinition of '_'")
struct P { int a, b; };
constexpr int f() {
  int _ = 1;
  int _ = 2;
  auto [x, _] = P{3, 4};
  auto g = [_ = 5] { return 6; };
  return x + g();
}
static_assert(f() == 9);
