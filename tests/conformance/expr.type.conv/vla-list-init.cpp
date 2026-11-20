// [dcl.init.list]/3, clang's VLA extension: a variable-length array type
// has no initializer form, so `V{}`, `decltype(a){}` and the compound
// literal `(int[n]){...}` are "variable-sized object may not be
// initialized".
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:17 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 24:17 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 23:12 note "declared here"
//@ diag: 24:17 error "variable-sized object may not be initialized"
//@ diag: 26:9 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 26:9 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 23:12 note "declared here"
//@ diag: 26:9 error "variable-sized object may not be initialized"
//@ diag: 30:14 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 30:14 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 29:12 note "declared here"
//@ diag: 30:9 error "variable-sized object may not be initialized"
//@ diag: 33:14 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 33:14 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 29:12 note "declared here"
//@ diag: 33:9 error "variable-sized object may not be initialized"
void f(int n) {
  typedef int V[n + 1];
  (void)V{};
  int a[n];
  (void)decltype(a){};
}
void g(int n) {
  (void)(int[n]){1, 2};
  const int m = 3;
  (void)(int[m]){1, 2};
  (void)(int[n]){};
}
