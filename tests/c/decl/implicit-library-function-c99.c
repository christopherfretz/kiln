// C99 6.5.1p2: an undeclared function is a constraint violation, and for
// one of clang's predefined library functions clang names its library
// type in the error (clang `LazilyCreateBuiltin`); a later declaration with
// another type is only a warning (`MergeFunctionDecl`).
//@ std: c17
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:25 error "call to undeclared library function 'labs' with type 'long (long)'; ISO C99 and later do not support implicit function declarations"
//@ diag: 12:25 note "include the header <stdlib.h> or explicitly provide a declaration for 'labs'"
//@ diag: 13:7 warning "incompatible redeclaration of library function 'labs'"
//@ diag: 12:25 note "'labs' is a builtin with type 'long (long)'"
long f(long x) { return labs(x); }
char *labs(int);
