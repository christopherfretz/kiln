// C17 6.6p6, 6.7.6.2p2: `sizeof` of a variable length array type is no
// constant, so at file scope (and in a static initializer, a case label,
// a static assertion, an enumerator) a `sizeof(int[m])` with a `const
// int m` is rejected as clang does; a compound literal whose bound
// cannot be folded is rejected, one whose bound folds is a warning.
// Found running GCC's C++ testsuite.
//@ std: c17
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:5 error "variable length array declaration not allowed at file scope"
//@ diag: 21:16 error "static assertion expression is not an integral constant expression"
//@ diag: 22:12 error "expression is not an integer constant expression"
//@ diag: 23:20 error "initializer element is not a compile-time constant"
//@ diag: 25:7 warning "variable length array folded to constant array as an extension"
//@ diag: 27:29 error "initializer element is not a compile-time constant"
//@ diag: 28:21 error "expression is not an integer constant expression"
//@ diag: 29:12 error "compound literal cannot be of variable-length array type"
//@ diag: 30:12 warning "variable length array folded to constant array as an extension"
const int m = 3;
int z[sizeof(int[m])];
_Static_assert(sizeof(int[m]) == 12, "");
enum { V = sizeof(int[m]) };
unsigned long s1 = sizeof(int[m]);
unsigned long s4 = sizeof(int[3]) + _Alignof(int[m]);
int (*cp)[m] = 0;
int f(int n) {
  static unsigned long s5 = sizeof(int[m]);
  switch (0) { case sizeof(int[m]): break; }
  int *a = (int[n]){1};
  int *b = (int[m]){1};
  return (int)s5 + a[0] + b[0];
}
