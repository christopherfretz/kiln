//@ strict-diags
//@ expect: compile-fail
//@ diag: 50:9 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 50:9 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 49:12 note "declared here"
//@ diag: 51:18 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 51:18 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 49:12 note "declared here"
//@ diag: 52:19 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 52:23 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 49:12 note "declared here"
//@ diag: 53:12 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 53:12 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 49:12 note "declared here"
//@ diag: 53:8 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 54:12 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 54:12 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 49:12 note "declared here"
//@ diag: 55:16 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 55:16 note "read of non-constexpr variable 'ca' is not allowed in a constant expression"
//@ diag: 47:20 note "declared here"
//@ diag: 55:14 warning "variable length array folded to constant array as an extension"
//@ diag: 56:16 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 56:16 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 49:12 note "declared here"
//@ diag: 56:14 error "variable length array declaration cannot have 'static' storage duration"
//@ diag: 57:22 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 57:22 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 49:12 note "declared here"
//@ diag: 57:20 error "variable length array declaration cannot have 'static' storage duration"
//@ diag: 58:16 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 58:16 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 49:12 note "declared here"
//@ diag: 58:14 error "variable length array declaration cannot have 'extern' linkage"
//@ diag: 59:9 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 59:9 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 49:12 note "declared here"
//@ diag: 59:9 error "variable-sized object may not be initialized"
//@ diag: 60:9 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 60:9 note "read of non-const variable 'gn' is not allowed in a constant expression"
//@ diag: 48:12 note "declared here"
// WP-183: a variable length array (GNU extension) in a function: a local
// variable, typedef or alias of one, an array of one and a pointer to one
// warn (-Wvla-cxx-extension, with the notes saying why the bound is not a
// constant expression). One with static storage duration folds its bound
// or is an error; one with external linkage or an initializer is an error.
const _Atomic(int) ca = 4;
extern int gn;
void f(int n) {
  int a[n];
  typedef char T[n + 1];
  using U = short[2 * n];
  int k[3][n];
  int (*p)[n] = nullptr;
  static int s[ca];
  static int t[n];
  thread_local int u[n];
  extern int e[n];
  int i[n] = {};
  int g[gn];
  (void)a; (void)k; (void)p; (void)s; (void)i; (void)g;
}
