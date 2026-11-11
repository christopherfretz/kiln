// C17 6.7.6.2p4 (new in C99): in C89 a variable length array is an
// extension, diagnosed under -pedantic (-Wvla-extension) once per runtime
// bound, at the bound. Not strict-diags: -pedantic C89 also warns about
// every `//` line of this header.
//@ std: c89
//@ args: -pedantic
//@ expect: compile-ok
//@ diag: 13:21 warning "variable length arrays are a C99 feature"
//@ diag: 15:9 warning "variable length arrays are a C99 feature"
//@ diag: 16:12 warning "variable length arrays are a C99 feature"
//@ diag: 16:8 warning "variable length arrays are a C99 feature"
//@ diag: 17:18 warning "variable length arrays are a C99 feature"
void f(int n, int a[n]);
void g(int n) {
  int v[n];
  int w[3][n];
  typedef char T[n + 1];
  (void)v; (void)w; (void)sizeof(T);
}
