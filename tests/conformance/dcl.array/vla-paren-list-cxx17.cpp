// Before C++20 a parenthesized list of several expressions is no
// initializer of an array, variable length or not: "array initializer
// must be an initializer list" at the declarator's name.
// Found running GCC's C++ testsuite.
//@ std: c++17
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:7 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 18:7 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 17:12 note "declared here"
//@ diag: 18:5 error "array initializer must be an initializer list"
//@ diag: 19:9 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 19:9 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 17:12 note "declared here"
//@ diag: 19:7 error "array initializer must be an initializer list"
struct S { S(); S(int); };
void f(int n) {
  S c[n](1, 2);
  int d[n](1, 2);
}
