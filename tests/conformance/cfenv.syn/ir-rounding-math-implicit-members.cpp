// [cfenv.syn], clang `-frounding-math` under a file-scope `FENV_ROUND
// FE_TONEAREST`: the state is not constrained there, so the implicit
// members first declared there (a default constructor declared at its
// use, an inheriting constructor, an explicitly defaulted comparison) are
// not `strictfp` and their FP operations are plain instructions, as the
// user-written functions there are; the functions defined before the
// pragma are `strictfp`, and an implicit constructor first declared in a
// block under `FENV_ACCESS ON` is (clang passes the state at the member's
// declaration as `UsesFPIntrin`). Compared with clang's IR exactly
// (spec/codegen.md "Floating-point semantics", WP-1808).
//@ expect: ir
//@ args: --target=aarch64-unknown-linux-gnu -std=c++23 -Wno-unknown-pragmas -frounding-math
double h(double);
struct D { double d = h(1.0) / 3.0; virtual void g() {} };
struct B { double d; B(double x) : d(x) {} };
#pragma STDC FENV_ROUND FE_TONEAREST
struct C { double d; bool operator==(const C&) const = default; };
struct I : B { using B::B; };
struct W { double d = h(2.0); };
bool cmp(C a, C b) { return a == b; }
double use() { D x; I i(1.0); return x.d + i.d; }
double blk() {
#pragma STDC FENV_ACCESS ON
  W w; return w.d;
}
