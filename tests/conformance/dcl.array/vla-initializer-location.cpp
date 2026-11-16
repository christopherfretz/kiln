// clang `FK_VariableLengthArrayHasInitializer`: a variable length array
// takes no initializer. A braced-init-list (with or without `=`) is
// reported at the bound; a single expression, `= e` or `(e)`, at the
// declarator's name. A parenthesized list of several expressions is a
// parenthesized aggregate initialization from C++20, reported at the
// bound like a braced one.
// Found running GCC's C++ testsuite.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 49:7 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 49:7 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 48:12 note "declared here"
//@ diag: 49:7 error "variable-sized object may not be initialized"
//@ diag: 50:7 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 50:7 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 48:12 note "declared here"
//@ diag: 50:5 error "variable-sized object may not be initialized"
//@ diag: 51:7 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 51:7 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 48:12 note "declared here"
//@ diag: 51:7 error "variable-sized object may not be initialized"
//@ diag: 52:9 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 52:9 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 48:12 note "declared here"
//@ diag: 52:9 error "variable-sized object may not be initialized"
//@ diag: 53:9 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 53:9 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 48:12 note "declared here"
//@ diag: 53:7 error "variable-sized object may not be initialized"
//@ diag: 54:9 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 54:9 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 48:12 note "declared here"
//@ diag: 54:7 error "variable-sized object may not be initialized"
//@ diag: 55:11 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 55:11 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 48:12 note "declared here"
//@ diag: 55:8 error "variable-sized object may not be initialized"
//@ diag: 56:7 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 56:7 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 48:12 note "declared here"
//@ diag: 56:7 error "variable-sized object may not be initialized"
//@ diag: 57:9 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 57:9 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 48:12 note "declared here"
//@ diag: 57:9 error "variable-sized object may not be initialized"
struct S { S(); S(int); };
void f(int n) {
  S b[n]{};
  S c[n](1);
  S d[n] = {};
  int e[n]{};
  int g[n](1);
  int a[n] = 1;
  int (h)[n](1);
  S i[n](1, 2);
  int j[n](1, 2);
}
