// [dcl.init.general]/16.6.2: a braced-init-list in a parenthesized
// initializer list-initializes the selected constructor's parameter.
#include "Inputs/initializer_list.h"
struct S { int a, b; };
struct T {
  int v;
  constexpr T(S s) : v(s.a * 10 + s.b) {}
};
constexpr T t({1, 2});
static_assert(t.v == 12);
struct V {
  int n;
  constexpr V(std::initializer_list<int> l) : n(int(l.size())) {}
};
constexpr V v({1, 2, 3});
static_assert(v.n == 3);
constexpr V w(V{4, 5});
static_assert(w.n == 2);
