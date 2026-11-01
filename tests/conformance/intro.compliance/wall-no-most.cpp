// [intro.compliance.general]/2, -Wall -Wno-most leaves the -Wall members
// outside -Wmost: -Wswitch and -Wvla-cxx-extension; -Wreturn-type,
// -Wmultichar and -Wtrigraphs are in -Wmost (WP-230). Probe: clang++-19
// -fsyntax-only -std=c++26 -Wno-everything -Wall -Wno-most on this file
// gives exactly the diagnostics below.
//@ expect: compile-ok
//@ strict-diags
//@ args: -Wno-everything -Wall -Wno-most
//@ diag: 25:11 warning "enumeration value 'e2' not handled in switch"
//@ diag: 26:9 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 26:9 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 20:11 note "declared here"
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
