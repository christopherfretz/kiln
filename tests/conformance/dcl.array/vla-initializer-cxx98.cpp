// Before C++11 a `{` after a declarator starts no initializer (clang's
// `ParseDeclarationAfterDeclarator`): the declaration ends with
// "expected ';' at end of declaration" and the braces are skipped, for
// a variable length array as for any other declarator. Before C++11
// the variable length array warning has no notes and is reported at
// the subexpression that is not an integral constant expression
// (clang's `CheckICE`). A parenthesized initializer is rejected at the
// declarator's name.
// Found running GCC's C++ testsuite.
//@ std: c++98
//@ expect: compile-fail
//@ strict-diags
//@ diag: 37:8 error "expected ';' at end of declaration"
//@ diag: 38:9 error "expected ';' at end of declaration"
//@ diag: 39:11 error "expected ';' at end of declaration"
//@ diag: 40:7 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 40:9 error "expected ';' at end of declaration"
//@ diag: 41:9 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 41:11 error "expected ';' at end of declaration"
//@ diag: 42:7 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 42:5 error "variable-sized object may not be initialized"
//@ diag: 43:7 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 43:7 error "variable-sized object may not be initialized"
//@ diag: 44:9 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 44:7 error "variable-sized object may not be initialized"
//@ diag: 45:13 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 46:9 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 47:18 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 48:12 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 48:8 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 49:17 warning "variable length arrays in C++ are a Clang extension"
struct S { S(); S(int); };
struct A { int x; };
const int k = 3;
int h(int);
void f(int n, A a) {
  int x{};
  S s[3]{};
  int z[k]{};
  S b[n]{};
  int e[n]{};
  S c[n](1);
  S d[n] = {};
  int g[n](1);
  int y[1 + h(1)];
  int w[a.x + 1];
  int m[2 * (1 + n)];
  int q[3][n];
  typedef int T[n + 1];
  (void)x; (void)s; (void)z;
}
