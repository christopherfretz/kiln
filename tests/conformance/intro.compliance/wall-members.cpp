// [intro.compliance.general]/2, -Wall = most, switch, switch-bool,
// vla-cxx-extension (kiln's subset of clang 19's All, in its order; WP-230).
// Probe: clang++-19 -fsyntax-only -std=c++26 -Wno-everything -Wall on this
// file gives exactly the diagnostics below (wmost-members.cpp plus -Wswitch
// and -Wvla-cxx-extension).
//@ expect: compile-ok
//@ strict-diags
//@ args: -Wno-everything -Wall
//@ diag: 23:12 warning "'/*' within block comment"
//@ diag: 24:9 warning "unknown pragma ignored"
//@ diag: 25:28 warning "field 'b' will be initialized after field 'a'"
//@ diag: 27:21 warning "'D::f' hides overloaded virtual function"
//@ diag: 26:25 note "hidden overloaded virtual function 'B::f' declared here: different number of parameters (1 vs 0)"
//@ diag: 28:35 warning "non-void function does not return a value in all control paths"
//@ diag: 31:11 warning "multi-character character constant"
//@ diag: 32:20 warning "trigraph ignored"
//@ diag: 35:11 warning "enumeration value 'e2' not handled in switch"
//@ diag: 36:9 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 36:9 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 30:11 note "declared here"
//@ diag: 38:25 warning "expression with side effects has no effect in an unevaluated context"
//@ diag: 33:7 warning "unused variable 'unused_var'"
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
