//@ expect: compile-ok
//@ strict-diags
// [dcl.attr.likelihood], [dcl.attr.fallthrough], [stmt.switch] (WP-182):
// constant evaluation reaches a case label written with a likelihood
// attribute, falls through a fallthrough statement, and evaluates
// attributed statements, also in an instantiation.
constexpr int f(int x) {
  int r = 0;
  switch (x) {
  [[likely]] case 1: r += 1; [[fallthrough]];
  [[unlikely]] case 2: r += 10; break;
  case 3: [[fallthrough]];
  [[likely]] default: r += 100;
  }
  if (r > 5) [[likely]] { r *= 2; } else [[unlikely]] r -= 1;
  while (r > 1000) [[unlikely]] r /= 2;
  [[likely]] return r;
}
static_assert(f(1) == 22);
static_assert(f(2) == 20);
static_assert(f(3) == 200);
static_assert(f(9) == 200);
template <class T> constexpr T g(T x) {
  switch (x) {
  [[unlikely]] case 0: return 7;
  [[likely]] default: break;
  }
  [[unlikely]] return x;
}
static_assert(g(0) == 7);
static_assert(g(5L) == 5);
