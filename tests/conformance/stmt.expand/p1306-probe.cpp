// [stmt.expand] (C++26, P1306R5): expansion statements `template for` over
// an expansion-init-list. WP-185 feature probe.
//@ expect: compile-ok
//@ strict-diags
//@ xfail(clang): neither clang 19 nor clang 22 implements expansion statements (P1306)
//@ xfail(kiln): expansion statements are not implemented (P1306)
constexpr int sum() {
  int s = 0;
  template for (constexpr int i : {1, 2, 3}) s += i;
  return s;
}
static_assert(sum() == 6);
