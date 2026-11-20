// [temp.arg.type], [temp.param]/10: a variably modified type (a GNU
// variable length array) is no template argument, a type parameter's
// default argument included, which is checked where it is written.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:25 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 18:25 note "read of non-const variable 'm' is not allowed in a constant expression"
//@ diag: 16:5 note "declared here"
//@ diag: 18:21 error "variably modified type 'int[m]' cannot be used as a template argument"
//@ diag: 19:7 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 19:7 note "read of non-const variable 'm' is not allowed in a constant expression"
//@ diag: 16:5 note "declared here"
//@ diag: 19:3 error "variably modified type 'int[m]' cannot be used as a template argument"

int m = 3;
template <class T> struct X {};
template <class T = int[m]> struct Y {};
X<int[m]> x;
