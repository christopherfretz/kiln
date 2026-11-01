// [expr.eq]/2, [expr.rel]/2 (C++26, P2865R6): equality and relational
// comparison of two arrays is ill-formed. WP-185 feature probe.
//@ expect: compile-fail
//@ strict-diags
//@ xfail(kiln): array comparison accepted without error
//@ xfail(clang): clang 19 only warns (P2865 is in clang 20)
//@ diag: 11:12 error "comparison between two arrays"
//@ diag: 11:12 warning "array comparison always evaluates to false"
int a[2], b[2];
bool f() {
  return a == b;
}
