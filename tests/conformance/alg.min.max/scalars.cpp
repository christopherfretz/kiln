// [alg.min.max]: std::min and std::max of two scalars return a reference
// to the smaller (larger) argument, the first one when neither is less
// than the other. The Oven evaluates the calls of two `const T&` of an
// arithmetic type with no comparator or a std::less / std::greater one
// natively (spec/oven.md "Library fast paths"); the value and the
// referenced object are the library's.
//@ expect: compile-ok
#include <algorithm>
#include <functional>
constexpr bool ints() {
  int a = 3, b = 5;
  if (std::min(a, b) != 3 || std::max(a, b) != 5)
    return false;
  if (&std::min(a, b) != &a || &std::max(a, b) != &b)
    return false;
  if (&std::min(b, a) != &a || &std::max(b, a) != &b)
    return false;
  // Equal arguments: both return the first.
  int c = 3;
  if (&std::min(a, c) != &a || &std::max(a, c) != &a)
    return false;
  return true;
}
static_assert(ints());
constexpr bool widths() {
  unsigned u = 4000000000u, v = 1;
  if (std::max(u, v) != 4000000000u || std::min(u, v) != 1)
    return false;
  long long n = -5, m = 3;
  if (std::min(n, m) != -5 || std::max(n, m) != 3)
    return false;
  signed char s = -1, t = 1;
  if (std::min(s, t) != -1)
    return false;
  unsigned char x = 255, y = 1;
  if (std::max(x, y) != 255)
    return false;
  bool f = false, g = true;
  if (std::min(f, g) || !std::max(f, g))
    return false;
  char p = 'a', q = 'z';
  return std::max(p, q) == 'z';
}
static_assert(widths());
constexpr bool floats() {
  double x = 1.5, y = -0.0, z = 0.0;
  if (std::min(x, y) != -0.0 || std::max(x, y) != 1.5)
    return false;
  // -0.0 and 0.0 are equal: the first argument.
  if (&std::min(y, z) != &y || &std::max(y, z) != &y)
    return false;
  // An unordered comparison is not less: the first argument.
  double nan = __builtin_nan("");
  if (&std::min(nan, x) != &nan || &std::min(x, nan) != &x)
    return false;
  float f = 2.0f, g = 1.0f;
  return std::min(f, g) == 1.0f && std::max(f, g) == 2.0f;
}
static_assert(floats());
constexpr bool comparators() {
  int a = 3, b = 5;
  if (std::min(a, b, std::less<>()) != 3 || std::max(a, b, std::less<int>()) != 5)
    return false;
  if (std::min(a, b, std::greater<>()) != 5 || std::max(a, b, std::greater<int>()) != 3)
    return false;
  if (&std::min(a, b, std::greater<int>()) != &b)
    return false;
  int c = 3;
  return &std::min(a, c, std::greater<>()) == &a && &std::max(a, c, std::greater<>()) == &a;
}
static_assert(comparators());
// Calls the Oven does not take natively run the library's code, with the
// same results.
enum E { e0, e1, e2 };
struct Count {
  int *n;
  constexpr bool operator()(int x, int y) const { return ++*n, x < y; }
};
constexpr bool others() {
  E a = e2, b = e1;
  if (std::min(a, b) != e1)
    return false;
  int n = 0, x = 4, y = 2;
  if (std::min(x, y, Count{&n}) != 2 || n != 1)
    return false;
  if (std::max(x, y, std::less<long>()) != 4)
    return false;
  const int *p = &x, *q = &y;
  return std::min(p, p) == p;
}
static_assert(others());
