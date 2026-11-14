// [except.spec]/2, [except.spec]/13: `noexcept(e)` on a function type,
// of a constant operand, of a value-dependent one, and of one naming the
// function's own parameters (libstdc++'s `swap` overloads). With
// `--pch-diff`, the header's snapshot carries each operand (WP-1300).
//@ expect: run
#include "Inputs/expr_noexcept.h"

struct Throws {
  Throws() noexcept(false) {}
  Throws &operator=(const Throws &) noexcept(false) { return *this; }
};
static_assert(noexcept(nx_sized()) && noexcept(nx_make<int>()) && !noexcept(nx_make<Throws>()));

int main() {
  nx_pair<int> a{1}, b{2};
  nx_pair<Throws> c, d;
  static_assert(noexcept(swap(a, b)) && !noexcept(swap(c, d)));
  swap(a, b);
  int i = 3;
  static_assert(noexcept(nx_add(i)));
  return a.a == 2 && b.a == 1 && nx_add(i) + nx_sized() + nx_make<int>() == 7 ? 0 : 1;
}
