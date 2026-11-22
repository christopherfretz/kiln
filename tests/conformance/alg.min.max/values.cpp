// [alg.min.max], [expr.call]/7: std::min and std::max read for their
// value. The Oven reads a local the register-locals scan keeps in a
// register, and a scalar temporary, where they are (spec/oven.md
// "Library fast paths"); each argument is still evaluated once, in
// order, and the call reads the arguments after all are evaluated.
//@ expect: compile-ok
#include <algorithm>
#include <functional>
constexpr int low(int n) {
  int m = n;
  for (int i = 0; i < n; ++i)
    m = std::min(m, n - i);
  return m;
}
static_assert(low(10) == 1);
constexpr int high(int n) {
  int m = 0;
  for (int i = 0; i < n; ++i)
    m = std::max(i * 3 % 7, m);
  return m;
}
static_assert(high(10) == 6);
constexpr int order() {
  int x = 5;
  // `x` binds to the variable, the second argument to a temporary holding
  // its old value; both are read after `x++`.
  int r = std::min(x, x++);
  int s = std::max(x, x--);
  return r * 100 + s * 10 + x;
}
static_assert(order() == 565);
constexpr int once() {
  int k = 0, x = 2;
  int r = std::min(x, ++k + 1);
  int s = std::max(k++, x);
  return r * 100 + s * 10 + k;
}
static_assert(once() == 222);
constexpr double mixed(double a) {
  double b = a * 2;
  double c[2] = {a, b};
  return std::max(b, c[0]) + std::min(c[1], a + 1);
}
static_assert(mixed(1.5) == 5.5);
constexpr long wide(long a, long b) {
  return std::min(a, b) - std::max(a, b);
}
static_assert(wide(-7, 9) == -16);
// A comparator form evaluates its comparator as the call does.
constexpr int with_cmp(int a, int b) {
  return std::max(a, b, std::greater<>()) + std::min(a, b, std::less<int>());
}
static_assert(with_cmp(3, 8) == 6);
