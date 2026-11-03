// [expr.prim.splice]/2, [expr.unary.op]/3 P2996R13: a splice-expression
// designating a non-static data member or member function is an
// id-expression without implicit member access, so `&[: r :]` forms a
// pointer to member. Run through codegen.
//@ expect: run
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ stdout: 7 9 14 16
#include "kiln_meta.h"
extern "C" int printf(const char*, ...);
struct S {
  int m;
  int n;
  int twice() const { return 2 * m; }
  virtual int plus(int a) const { return n + a; }
};
struct D : S {
  int plus(int a) const override { return 100 + a; }
};

int main() {
  S s;
  s.m = 7;
  s.n = 9;
  int S::* pm = &[: ^^S::m :];
  int S::* pn = &[: ^^S::n :];
  int (S::*pf)() const = &[: ^^S::twice :];
  int (S::*pv)(int) const = &[: ^^S::plus :];
  D d;
  d.n = 0;
  printf("%d %d %d %d\n", s.*pm, s.*pn, (s.*pf)(), (s.*pv)(7) + (d.*pv)(0) - 100);
  return 0;
}
