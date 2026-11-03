//@ strict-diags
//@ expect: compile-fail
//@ diag: 29:7 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 29:7 note "read of non-const variable 'n' is not allowed in a constant expression"
//@ diag: 26:5 note "declared here"
//@ diag: 29:5 error "variable length array declaration not allowed at file scope"
//@ diag: 30:14 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 30:14 note "read of non-const variable 'm' is not allowed in a constant expression"
//@ diag: 27:12 note "declared here"
//@ diag: 30:12 error "variable length array declaration not allowed at file scope"
//@ diag: 31:7 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 31:7 note "non-constexpr function 'g' cannot be used in a constant expression"
//@ diag: 28:5 note "declared here"
//@ diag: 31:5 error "variable length array declaration not allowed at file scope"
//@ diag: 32:21 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 32:21 note "read of non-const variable 'n' is not allowed in a constant expression"
//@ diag: 26:5 note "declared here"
//@ diag: 32:19 error "variable length array declaration not allowed at file scope"
//@ diag: 33:15 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 33:15 note "read of non-const variable 'n' is not allowed in a constant expression"
//@ diag: 26:5 note "declared here"
//@ diag: 33:13 error "variable length array declaration not allowed at file scope"
// [dcl.array]/1: an array bound is a converted constant expression; a
// namespace-scope array whose bound is not one (and does not fold) is
// clang's variable length array, rejected at the declarator's name.
int n = 3;
extern int m;
int g(int k) { return k; }
int a[n];
static int b[m];
int d[g(2)];
namespace N { int e[n]; }
typedef int T[n];
