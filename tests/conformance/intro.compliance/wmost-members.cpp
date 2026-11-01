// [intro.compliance.general]/2 (diagnostics beyond those required), -Wmost:
// the subgroups kiln implements, in clang 19's order: comment, multichar,
// range-loop-construct, reorder, return-type, trigraphs, uninitialized,
// unknown-pragmas, unused (> unused-value > unevaluated-expression),
// overloaded-virtual (WP-230). switch and vla-cxx-extension are in -Wall,
// not -Wmost. Probe: clang++-19 -fsyntax-only -std=c++26 -Wno-everything
// -Wmost on this file gives exactly the diagnostics below.
// range-loop-construct has no emitted diagnostic in kiln yet.
//@ expect: compile-ok
//@ strict-diags
//@ args: -Wno-everything -Wmost
//@ diag: 22:12 warning "'/*' within block comment"
//@ diag: 23:9 warning "unknown pragma ignored"
//@ diag: 24:28 warning "field 'b' will be initialized after field 'a'"
//@ diag: 26:21 warning "'D::f' hides overloaded virtual function"
//@ diag: 25:25 note "hidden overloaded virtual function 'B::f' declared here: different number of parameters (1 vs 0)"
//@ diag: 27:35 warning "non-void function does not return a value in all control paths"
//@ diag: 30:11 warning "multi-character character constant"
//@ diag: 31:20 warning "trigraph ignored"
//@ diag: 37:25 warning "expression with side effects has no effect in an unevaluated context"
//@ diag: 32:7 warning "unused variable 'unused_var'"
/* comment /* nested */
#pragma kiln_unknown_pragma
struct R { int a, b; R() : b(1), a(2) {} };
struct B { virtual void f(int); };
struct D : B { void f(); };
int ret(int i) { if (i) return 1; }
int self_init = self_init;
int g(int n) {
  int m = 'ab';
  const char *t = "??=";
  int unused_var;
  enum E { e1, e2 } e = e1;
  switch (e) { case e1: break; }
  int v[n]; (void)v;
  (void)t;
  return m + (int)sizeof(n++);
}
