//@ strict-diags
//@ expect: compile-fail
//@ diag: 32:16 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 32:16 note "read of non-const variable 'n' is not allowed in a constant expression"
//@ diag: 29:5 note "declared here"
//@ diag: 32:14 error "variable length array declaration not allowed at file scope"
//@ diag: 33:9 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 33:9 note "read of non-const variable 'n' is not allowed in a constant expression"
//@ diag: 29:5 note "declared here"
//@ diag: 33:7 error "fields must have a constant size: 'variable length array in structure' extension will never be supported"
//@ diag: 34:12 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 34:12 note "read of non-const variable 'n' is not allowed in a constant expression"
//@ diag: 29:5 note "declared here"
//@ diag: 34:9 error "fields must have a constant size: 'variable length array in structure' extension will never be supported"
//@ diag: 35:19 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 35:19 note "read of non-const variable 'n' is not allowed in a constant expression"
//@ diag: 29:5 note "declared here"
//@ diag: 35:16 error "variably modified type declaration not allowed at file scope"
//@ diag: 37:19 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 37:19 note "read of non-const variable 'n' is not allowed in a constant expression"
//@ diag: 29:5 note "declared here"
//@ diag: 37:15 error "variably modified type declaration not allowed at file scope"
//@ diag: 38:12 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 38:12 note "read of non-const variable 'n' is not allowed in a constant expression"
//@ diag: 29:5 note "declared here"
//@ diag: 38:7 error "function declaration cannot have variably modified type"
// [dcl.array]/1, [class.mem.general]: a non-constant bound in a static
// data member, a non-static data member, a pointer and a function type.
int n = 3;
// (clang: err_vla_decl_in_file_scope, err_typecheck_field_variable_size.)
struct S {
  static int c[n];
  int f[n];
  int (*p)[n];
  static int (*q)[n];
};
typedef int (*TP)[n];
int (*h())[n];
