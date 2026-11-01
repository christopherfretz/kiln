// [dcl.array]/1, Clang extension: in a GNU dialect a VLA's
// 'variable length arrays in C++ are a Clang extension' is off by default
// but -Wall enables it, vla-cxx-extension being a subgroup of all (clang 19
// DiagnosticGroups.td All; WP-230). Probe: clang++-19 -fsyntax-only
// -std=gnu++26 -Wall on this file gives exactly the diagnostics below.
//@ expect: compile-ok
//@ strict-diags
//@ args: -std=gnu++26 -Wall
//@ diag: 13:9 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 13:9 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 12:12 note "declared here"
void g(int n) {
  int a[n];
  (void)a;
}
