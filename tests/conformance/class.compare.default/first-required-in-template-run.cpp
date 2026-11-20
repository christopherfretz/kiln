//@ expect: run
//@ args: -std=c++26
//@ stdout: 1 1 0 1|
// A defaulted comparison of a non-template class first required inside a
// template's definition (a non-dependent `a < b` there) is defined as a
// member of that class, not as part of the template: the immediate
// invocations of its body (the consteval `0` conversions of `cmp != 0`)
// are evaluated ([expr.const]/25), and the constant evaluation of a
// variable's initializer that instantiates the template may define it.
// Reduced from real code; both failed before WP-1738 (an undefined
// reference to the comparison category's consteval constructor, and an
// internal error).
#include <compare>
#include <cstdio>

struct V {
  int v;
  std::strong_ordering operator<=>(const V &) const = default;
};

struct W {
  int v;
  std::weak_ordering operator<=>(const W &) const = default;
};

template <class T> int all(T seed) {
  V a{seed}, b{3};
  int r = 0;
  r += (a < b) + (a > b);
  return r;
}

template <class T> int weak(T seed) {
  W a{seed}, b{3};
  return (a <= b) + (a == b);
}

int x = all(7);
int y = weak(3);

int main() {
  std::printf("%d %d %d %d|\n", x, all(1), weak(9), weak(3) == 2);
}
