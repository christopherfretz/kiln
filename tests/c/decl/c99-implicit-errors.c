// C99 6.5.1p2, 6.7.2p2: an undeclared function and a declaration without
// a type specifier are constraint violations from C99; clang 19 makes
// -Wimplicit-function-declaration and -Wimplicit-int errors there. A
// parameter or block-scope `auto` with no type specifier is the same
// implicit int (not an unknown type name), reported at the name.
//@ expect: compile-fail
//@ std: c99
//@ strict-diags
//@ diag: 15:8 error "type specifier missing, defaults to 'int'; ISO C99 and later do not support implicit int"
//@ diag: 16:22 error "call to undeclared function 'g'; ISO C99 and later do not support implicit function declarations"
//@ diag: 17:16 error "type specifier missing, defaults to 'int'; ISO C99 and later do not support implicit int"
//@ diag: 17:25 error "type specifier missing, defaults to 'int'; ISO C99 and later do not support implicit int"
//@ diag: 18:20 error "type specifier missing, defaults to 'int'; ISO C99 and later do not support implicit int"
//@ diag: 19:10 error "illegal storage class on file-scoped variable"
static counter = 7;
int f(void) { return g(1); }
int p(register r, const q) { return r + q; }
int u(void) { auto a = 1; auto int b = 2; return a + b; }
auto int file_auto;
