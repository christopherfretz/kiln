// [intro.compliance.general]/2, unevaluated-expression is a subgroup of
// unused-value, not of most directly: -Wmost -Wno-unused-value turns off
// 'expression with side effects has no effect in an unevaluated context'
// (WP-230). Probe: clang++-19 -fsyntax-only -std=c++26 -Wno-everything
// -Wmost -Wno-unused-value on this file gives exactly the diagnostics
// below.
//@ expect: compile-ok
//@ strict-diags
//@ args: -Wno-everything -Wmost -Wno-unused-value
//@ diag: 19:12 warning "'/*' within block comment"
//@ diag: 20:9 warning "unknown pragma ignored"
//@ diag: 21:28 warning "field 'b' will be initialized after field 'a'"
//@ diag: 23:21 warning "'D::f' hides overloaded virtual function"
//@ diag: 22:25 note "hidden overloaded virtual function 'B::f' declared here: different number of parameters (1 vs 0)"
//@ diag: 24:35 warning "non-void function does not return a value in all control paths"
//@ diag: 27:11 warning "multi-character character constant"
//@ diag: 28:20 warning "trigraph ignored"
//@ diag: 29:7 warning "unused variable 'unused_var'"
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
