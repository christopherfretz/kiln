// [dcl.array]/1, [dcl.typedef]/2: an array bound is a converted constant
// expression. At namespace and class scope clang's variable length array
// extension rejects the declaration at its name (an alias-declaration like a
// typedef, folding a bound that folds), after warning on every array around
// the variable length one.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 93:16 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 93:16 note "read of non-const variable 'n' is not allowed in a constant expression"
//@ diag: 89:5 note "declared here"
//@ diag: 93:13 error "variable length array declaration not allowed at file scope"
//@ diag: 94:16 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 94:16 note "read of non-const variable 'n' is not allowed in a constant expression"
//@ diag: 89:5 note "declared here"
//@ diag: 94:7 error "variable length array declaration not allowed at file scope"
//@ diag: 95:33 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 95:33 note "read of non-const variable 'n' is not allowed in a constant expression"
//@ diag: 89:5 note "declared here"
//@ diag: 95:26 error "variable length array declaration not allowed at file scope"
//@ diag: 96:26 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 96:26 note "read of non-const variable 'n' is not allowed in a constant expression"
//@ diag: 89:5 note "declared here"
//@ diag: 96:24 error "variable length array declaration not allowed at file scope"
//@ diag: 96:45 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 96:45 note "read of non-const variable 'n' is not allowed in a constant expression"
//@ diag: 89:5 note "declared here"
//@ diag: 96:36 error "variable length array declaration not allowed at file scope"
//@ diag: 98:16 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 98:21 note "read of non-constexpr variable 'cd' is not allowed in a constant expression"
//@ diag: 97:14 note "declared here"
//@ diag: 98:7 warning "variable length array folded to constant array as an extension"
//@ diag: 102:10 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 102:10 note "read of non-const variable 'n' is not allowed in a constant expression"
//@ diag: 89:5 note "declared here"
//@ diag: 102:6 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 102:5 error "variable length array declaration not allowed at file scope"
//@ diag: 103:7 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 103:7 note "read of non-const variable 'n' is not allowed in a constant expression"
//@ diag: 89:5 note "declared here"
//@ diag: 103:5 error "variable length array declaration not allowed at file scope"
//@ diag: 104:10 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 104:10 note "read of non-const variable 'n' is not allowed in a constant expression"
//@ diag: 89:5 note "declared here"
//@ diag: 104:7 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 104:7 note "read of non-const variable 'n' is not allowed in a constant expression"
//@ diag: 89:5 note "declared here"
//@ diag: 104:5 error "variable length array declaration not allowed at file scope"
//@ diag: 105:19 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 105:19 note "read of non-const variable 'n' is not allowed in a constant expression"
//@ diag: 89:5 note "declared here"
//@ diag: 105:15 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 105:13 error "variable length array declaration not allowed at file scope"
//@ diag: 106:19 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 106:19 note "read of non-const variable 'n' is not allowed in a constant expression"
//@ diag: 89:5 note "declared here"
//@ diag: 106:15 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 106:7 error "variable length array declaration not allowed at file scope"
//@ diag: 107:21 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 107:21 note "read of non-const variable 'n' is not allowed in a constant expression"
//@ diag: 89:5 note "declared here"
//@ diag: 107:17 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 107:16 error "fields must have a constant size: 'variable length array in structure' extension will never be supported"
//@ diag: 107:34 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 107:34 note "read of non-const variable 'n' is not allowed in a constant expression"
//@ diag: 89:5 note "declared here"
//@ diag: 107:31 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 107:31 note "read of non-const variable 'n' is not allowed in a constant expression"
//@ diag: 89:5 note "declared here"
//@ diag: 107:29 error "fields must have a constant size: 'variable length array in structure' extension will never be supported"
//@ diag: 108:11 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 108:11 note "read of non-const variable 'n' is not allowed in a constant expression"
//@ diag: 89:5 note "declared here"
//@ diag: 108:7 error "variably modified type declaration not allowed at file scope"
//@ diag: 109:15 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 109:15 note "read of non-const variable 'n' is not allowed in a constant expression"
//@ diag: 89:5 note "declared here"
//@ diag: 109:7 error "variably modified type declaration not allowed at file scope"
//@ diag: 110:21 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 110:21 note "read of non-const variable 'n' is not allowed in a constant expression"
//@ diag: 89:5 note "declared here"
//@ diag: 110:17 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 110:42 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 110:42 note "read of non-const variable 'n' is not allowed in a constant expression"
//@ diag: 89:5 note "declared here"
//@ diag: 110:39 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 110:39 note "read of non-const variable 'n' is not allowed in a constant expression"
//@ diag: 89:5 note "declared here"
int n = 3;

// An alias-declaration at namespace or class scope is a typedef: diagnosed
// at its name.
typedef int T1[n];
using T2 = int[n];
template <class T> using T3 = T[n];
struct S { typedef int M[n]; using M2 = int[n]; };
const double cd = 2;
using T4 = int[(int)cd];
T4 t4;

// The arrays around a variable length array are ones too, each warned.
int z[2][n];
int y[n][2];
int w[n][n];
typedef int T5[2][n];
using T6 = int[2][n];
struct F { int a[2][n]; int b[n][n]; };
int (*pv)[n];
int (*apv[2])[n];
void f() { int q[2][n]; using L = int[n][n]; }
