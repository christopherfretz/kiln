// C99 6.5.1p2, 6.7.2p2: an undeclared function and a declaration without
// a type specifier are constraint violations from C99; clang 19 makes
// -Wimplicit-function-declaration and -Wimplicit-int errors there.
//@ expect: compile-fail
//@ std: c99
//@ strict-diags
//@ diag: 9:8 error "type specifier missing, defaults to 'int'; ISO C99 and later do not support implicit int"
//@ diag: 10:22 error "call to undeclared function 'g'; ISO C99 and later do not support implicit function declarations"
static counter = 7;
int f(void) { return g(1); }
