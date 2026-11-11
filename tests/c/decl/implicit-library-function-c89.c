// C89 3.3.2.2 (implicit `extern int f()`) with clang's predefined library
// functions: an undeclared call to one declares it with its library type,
// not `int ()` (clang `LazilyCreateBuiltin`), with a warning naming the
// header; a later declaration with another type is only warned about
// (`MergeFunctionDecl`, -Wincompatible-library-redeclaration) and takes
// no -Wdeprecated-non-prototype diagnostics. Size-typed functions print
// `__size_t` from clang 22, so this test keeps to `int` and `double`.
//@ args: -std=gnu89
//@ expect: compile-ok
//@ strict-diags
//@ diag: 22:7 warning "implicitly declaring library function 'abs' with type 'int (int)'"
//@ diag: 22:7 note "include the header <stdlib.h> or explicitly provide a declaration for 'abs'"
//@ diag: 23:14 warning "implicitly declaring library function 'abort' with type 'void (void) __attribute__((noreturn))'"
//@ diag: 23:14 note "include the header <stdlib.h> or explicitly provide a declaration for 'abort'"
//@ diag: 24:3 warning "implicitly declaring library function 'printf' with type 'int (const char *, ...)'"
//@ diag: 24:3 note "include the header <stdio.h> or explicitly provide a declaration for 'printf'"
//@ diag: 25:10 warning "implicitly declaring library function 'fabs' with type 'double (double)'"
//@ diag: 25:10 note "include the header <math.h> or explicitly provide a declaration for 'fabs'"
//@ diag: 27:6 warning "incompatible redeclaration of library function 'abs'"
//@ diag: 22:7 note "'abs' is a builtin with type 'int (int)'"
int f(int i) {
  i = abs(i);
  if (i < 0) abort();
  printf("%d\n", i);
  return fabs(-1.5) > 1.0;
}
long abs();
double fabs(double);
