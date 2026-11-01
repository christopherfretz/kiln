//@ strict-diags
//@ expect: compile-fail
//@ diag: 28:9 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 28:9 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 27:29 note "declared here"
//@ diag: 34:9 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 34:9 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 33:24 note "declared here"
//@ diag: 37:15 error "static assertion expression is not an integral constant expression"
//@ diag: 35:10 note "subexpression not valid in a constant expression"
//@ diag: 37:15 note "in call to 'size(3)'"
//@ diag: 39:9 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 39:9 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 38:25 note "declared here"
//@ diag: 43:15 error "static assertion expression is not an integral constant expression"
//@ diag: 40:8 note "assignment to element of array without known bound is not allowed in a constant expression"
//@ diag: 43:15 note "in call to 'write(3)'"
//@ diag: 45:9 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 45:9 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 44:24 note "declared here"
//@ diag: 48:15 error "static assertion expression is not an integral constant expression"
//@ diag: 46:10 note "read of element of array without known bound is not allowed in a constant expression"
//@ diag: 48:15 note "in call to 'read(3)'"
// WP-183: a constexpr function may declare a variable length array (GNU
// extension), but its size is not a constant and its elements cannot be
// accessed in a constant expression.
constexpr int decl_only(int n) {
  int a[n];
  (void)a;
  return 1;
}
static_assert(decl_only(3) == 1);
constexpr int size(int n) {
  int a[n];
  return sizeof(a);
}
static_assert(size(3) == 12);
constexpr int write(int n) {
  int a[n];
  a[0] = 1;
  return 1;
}
static_assert(write(3) == 1);
constexpr int read(int n) {
  int a[n];
  return a[1];
}
static_assert(read(3) == 0);
