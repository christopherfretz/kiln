// C17 6.9.1p6-p7, 6.5.2.2p6, 6.7.6.3p14: a K&R definition (identifier list
// plus declaration list) has no prototype (clang checks a call's count);
// an identifier list outside a definition is an error.
//@ expect: compile-fail
//@ std: c17
//@ strict-diags
//@ diag: 15:5 warning "a function definition without a prototype is deprecated in all versions of C and is not supported in C23"
//@ diag: 16:9 error "parameter 'y' was not declared, defaults to 'int'; ISO C99 and later do not support implicit int"
//@ diag: 16:5 warning "a function definition without a prototype is deprecated in all versions of C and is not supported in C23"
//@ diag: 17:28 warning "too few arguments in call to 'add'"
//@ diag: 18:34 warning "too many arguments in call to 'add'"
//@ diag: 19:7 error "a parameter list without types is only allowed in a function definition"
//@ diag: 20:21 error "parameter named 'q' is missing"
//@ diag: 20:5 warning "a function definition without a prototype is deprecated in all versions of C and is not supported in C23"
int add(a, b) int a; long b; { return a + (int)b; }
int dbl(y) { return y * 2; }
int f1(void) { return add(1); }
int f2(void) { return add(1, 2, 3); }
int g(p);
int h(p) int p; int q; { return p; }
