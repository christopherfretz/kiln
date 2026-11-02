// [expr.new]/23, [dcl.init.general]/16.5 (P0960, P1975): an array
// new-expression may have a parenthesized expression-list; the array is
// aggregate-initialized from it, each element copy-initialized from its
// braced-init-list and the rest value-initialized. In a template the
// instantiation re-forms the same initializer (temp.res/subst-new-expression.cpp).
// clang 19 agrees; clang 22 rejects it, but only in a template ("array 'new'
// cannot have initialization arguments"; the same expression outside a
// template is accepted), which is a clang 22 regression, so the oracle is
// not consulted and kiln follows the standard.
//@ expect: run
//@ skip(clang): clang 22 rejects a parenthesized array new in a template (regression; [expr.new]/23)
//@ stdout: 46 46
#include <cstdio>

struct S {
  int v;
  constexpr S(int a, int b = 0) : v(a * 10 + b) {}
  constexpr S() : v(5) {}
};

template <int K> constexpr int g() {
  int a = K;
  S* s = new S[3]({a, 1}, {a});   // 21 20 5
  int r = s[0].v + s[1].v + s[2].v;
  delete[] s;
  return r;
}
static_assert(g<2>() == 21 + 20 + 5);

int f() {
  int a = 2;
  S* s = new S[3]({a, 1}, {a});
  int r = s[0].v + s[1].v + s[2].v;
  delete[] s;
  return r;
}

int main() { std::printf("%d %d\n", g<2>(), f()); }
