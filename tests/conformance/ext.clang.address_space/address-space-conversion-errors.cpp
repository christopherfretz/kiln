// The negative twins of address-space-conversions.cpp (WP-1803): no
// implicit conversion leaves a pointee's space or enters one that does not
// hold it, no named cast but a C-style one crosses spaces, pointers to
// unrelated spaces are distinct pointer types, and a space differs only at
// the first pointee level. clang wording.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:57 error "cannot initialize a variable of type 'int *' with an lvalue of type '__attribute__((address_space(1))) int *'"
//@ diag: 28:57 error "cannot initialize a variable of type '__attribute__((address_space(1))) int *' with an lvalue of type 'int *'"
//@ diag: 29:91 error "cannot initialize a variable of type '__attribute__((address_space(2))) int *' with an lvalue of type '__attribute__((address_space(1))) int *'"
//@ diag: 30:58 error "cannot initialize a variable of type 'void *' with an lvalue of type '__attribute__((address_space(1))) int *'"
//@ diag: 31:61 error "static_cast from '__attribute__((address_space(1))) int *' to 'int *' is not allowed"
//@ diag: 32:61 error "reinterpret_cast from '__attribute__((address_space(1))) int *' to 'int *' is not allowed"
//@ diag: 33:61 error "const_cast from '__attribute__((address_space(1))) int *' to 'int *' is not allowed"
//@ diag: 34:69 error "comparison of distinct pointer types ('__attribute__((address_space(1))) int *' and 'int *')"
//@ diag: 35:103 error "comparison of distinct pointer types ('__attribute__((address_space(1))) int *' and '__attribute__((address_space(2))) int *')"
//@ diag: 36:55 error "cannot initialize a variable of type 'int *' with an lvalue of type '__generic int *'"
//@ diag: 37:84 error "cannot initialize a variable of type '__global int *' with an lvalue of type '__local int *'"
//@ diag: 38:88 error "cannot initialize a variable of type '__generic int *' with an lvalue of type '__constant int *'"
//@ diag: 39:92 error "cannot initialize a variable of type '__global_device int *' with an lvalue of type '__global int *'"
//@ diag: 40:55 error "cannot initialize a variable of type 'int *' with an lvalue of type '__private int *'"
//@ diag: 41:57 error "cannot initialize a variable of type 'int *' with an lvalue of type '__attribute__((address_space(0))) int *'"
//@ diag: 42:57 error "cannot initialize a variable of type '__attribute__((address_space(0))) int *' with an lvalue of type 'int *'"
//@ diag: 43:59 error "cannot initialize a variable of type 'int **' with an lvalue of type '__attribute__((address_space(1))) int **'"
//@ diag: 44:88 error "cannot initialize a variable of type '__generic int **' with an lvalue of type '__global int **'"

void a(__attribute__((address_space(1))) int *p) { int *q = p; }
void b(int *p) { __attribute__((address_space(1))) int *q = p; }
void c(__attribute__((address_space(1))) int *p) { __attribute__((address_space(2))) int *q = p; }
void v(__attribute__((address_space(1))) int *p) { void *q = p; }
void s(__attribute__((address_space(1))) int *p) { int *q = static_cast<int *>(p); }
void r(__attribute__((address_space(1))) int *p) { int *q = reinterpret_cast<int *>(p); }
void k(__attribute__((address_space(1))) int *p) { int *q = const_cast<int *>(p); }
bool e(__attribute__((address_space(1))) int *p, int *q) { return p == q; }
bool f(__attribute__((address_space(1))) int *p, __attribute__((address_space(2))) int *q) { return p == q; }
void g(__attribute__((opencl_generic)) int *p) { int *q = p; }
void h(__attribute__((opencl_local)) int *p) { __attribute__((opencl_global)) int *q = p; }
void i(__attribute__((opencl_constant)) int *p) { __attribute__((opencl_generic)) int *q = p; }
void j(__attribute__((opencl_global)) int *p) { __attribute__((opencl_global_device)) int *q = p; }
void l(__attribute__((opencl_private)) int *p) { int *q = p; }
void m(__attribute__((address_space(0))) int *p) { int *q = p; }
void n(int *p) { __attribute__((address_space(0))) int *q = p; }
void o(__attribute__((address_space(1))) int **p) { int **q = p; }
void t(__attribute__((opencl_global)) int **p) { __attribute__((opencl_generic)) int **q = p; }
