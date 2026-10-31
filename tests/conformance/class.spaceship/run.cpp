// [class.spaceship]/1, [class.compare.secondary]: a defaulted `<=>` and
// the secondary operators rewritten to it, called at run time (code
// generation of the implicit definitions).
//@ expect: run
//@ stdout: -1 0 1 1 0 1 1
#include "../cmp.categories/Inputs/compare.h"
extern "C" int printf(const char*, ...);
struct B { long b; auto operator<=>(const B&) const = default; };
struct S : B {
  int a[2];
  double d;
  auto operator<=>(const S&) const = default;
  bool operator<(const S&) const = default;
};
int sign(std::partial_ordering o) { return o < 0 ? -1 : o > 0 ? 1 : 0; }
int main(int argc, char**) {
  S x{{1}, {2, 3}, 0.5}, y = x, z = x;
  y.a[1] = argc + 3;
  z.b = argc - 1;
  printf("%d %d %d %d %d %d %d\n", sign(x <=> y), sign(x <=> x), sign(x <=> z),
         x < y, y < x, x == x, x != y);
  return 0;
}
