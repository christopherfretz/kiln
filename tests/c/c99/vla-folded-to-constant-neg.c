// C17 6.7.6.2p2, 6.7.9p3: a variable length array whose bound does not
// fold (it reads a non-`const` object or a parameter) is not folded to a
// constant array: at file scope it is an error, with an initializer it is
// "variable-sized object may not be initialized", and an `extern` one
// cannot have linkage. The `const int` bound beside them folds
// (vla-folded-to-constant.c). Found running GCC's C++ testsuite.
//@ std: c17
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:5 error "variable length array declaration not allowed at file scope"
//@ diag: 17:9 error "variable-sized object may not be initialized"
//@ diag: 18:9 error "variable-sized object may not be initialized"
//@ diag: 19:14 error "variable length array declaration cannot have 'extern' linkage"
int n = 3;
int bad[n];
void f(int p) {
  int b[n] = {1};
  int c[p] = {0};
  extern int e[p];
  (void)b; (void)c;
}
