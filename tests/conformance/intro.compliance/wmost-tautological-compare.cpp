// [intro.compliance.general]/2 (diagnostics beyond those required), -Wmost:
// tautological-compare is a subgroup of most (clang 19's order: after
// return-type, before trigraphs; WP-356), so the array comparison that
// always evaluates to false ([expr.eq]/2, [depr.array.comp]) warns under
// -Wno-everything -Wmost while the deprecation itself does not. Probe:
// clang++-19 -fsyntax-only -std=c++20 -Wno-everything -Wmost.
//@ expect: compile-ok
//@ std: c++20
//@ strict-diags
//@ args: -Wno-everything -Wmost
//@ diag: 15:21 warning "array comparison always evaluates to false"
//@ diag: 16:21 warning "array comparison always evaluates to true"

int a[2], b[2];
bool f() { return a == b; }
bool g() { return a != b; }
