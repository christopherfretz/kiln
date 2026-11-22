// C23 7.6.1, [cfenv.syn]: ir-fenv-access-implicit-members.cpp on x86-64:
// an implicit member is `strictfp` when the FP
// state where it is first declared is constrained (clang passes it as
// `UsesFPIntrin`), wherever it is defined: the default constructor of a
// class used first in a block under `FENV_ACCESS ON` is, and so are its
// FP operations (constrained calls); one used first outside is not, nor
// is the destructor a polymorphic class declares at its `}`. Compared
// with clang's IR exactly (spec/codegen.md "Floating-point semantics",
// WP-1808).
//@ expect: ir
//@ args: --target=x86_64-unknown-linux-gnu -std=c++23 -Wno-unknown-pragmas
double h(double);
struct W { double d = h(2.0) * 2.0; };
struct V { double d = h(3.0) + 1.0; };
struct Q { ~Q(); };
struct P { Q q; double d = h(4.0) - 1.0; virtual void g() {} };
double plain() { V v; return v.d; }
double blk() {
#pragma STDC FENV_ACCESS ON
  W w;
  V v;
  P p;
  return w.d + v.d + p.d;
}
