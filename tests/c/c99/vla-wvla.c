// C17 6.7.6.2p4: variable length arrays are standard C99; clang's -Wvla
// ("variable length array used") is off by default and not in -Wall, so it
// fires only when asked for -- once per runtime bound, at the bound.
//@ std: c17
//@ args: -Wvla
//@ expect: compile-ok
//@ strict-diags
//@ diag: 13:21 warning "variable length array used"
//@ diag: 15:9 warning "variable length array used"
//@ diag: 16:12 warning "variable length array used"
//@ diag: 16:8 warning "variable length array used"
//@ diag: 17:18 warning "variable length array used"
void f(int n, int a[n]);
void g(int n) {
  int v[n];
  int w[3][n];
  typedef char T[n + 1];
  (void)v; (void)w; (void)sizeof(T);
}
