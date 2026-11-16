// [dcl.init.general]/16.5, [expr.new]/23 (P0960, P1009): from C++20 an
// array new-expression is initialized from a parenthesized expression list,
// with a deduced bound when none is written; class elements are
// copy-initialized from their arguments (no default constructor is needed
// when the arguments meet the bound), also in a constant expression.
//@ std: c++20
//@ expect: run
//@ stdout: 4 5 6 7 8 9
#include <stdio.h>
struct S { int a; };
struct C { int v; C(int x) : v(x) {} };
constexpr int f() {
  int *p = new int[3](1, 2);
  int r = p[0] * 100 + p[1] * 10 + p[2];
  delete[] p;
  return r;
}
static_assert(f() == 120, "");
int main() {
  int *a = new int[](4, 5);
  S *s = new S[2]({6}, {7});
  C *c = new C[2](8, 9);
  printf("%d %d %d %d %d %d\n", a[0], a[1], s[0].a, s[1].a, c[0].v, c[1].v);
  delete[] a;
  delete[] s;
  delete[] c;
}
