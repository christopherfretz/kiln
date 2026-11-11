// C17 6.5.16.1p1: assigning between pointers to incompatible types is a
// constraint violation; clang 22 (-fclang-compat=22) makes it an error by default
// (-Wincompatible-pointer-types, WP-1285); dropping qualifiers stays a warning.
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:7 error "incompatible pointer types initializing 'long *' with an expression of type 'int *'"
//@ diag: 18:24 warning "returning 'const char *' from a function with result type 'char *' discards qualifiers"
//@ diag: 21:6 error "incompatible pointer types assigning to 'float *' from 'int *'"
//@ diag: 22:5 error "incompatible pointer types passing 'int *' to parameter of type 'long *'"
//@ diag: 16:14 note "passing argument to parameter 'p' here"
//@ diag: 23:9 warning "initializing 'char *' with an expression of type 'const char *' discards qualifiers"
//@ diag: 24:9 error "incompatible pointer types initializing 'int **' with an expression of type 'float **'"
int i;
long *lp = &i;
void f(long *p);
const char *cs;
char *g(void) { return cs; }
int main(void) {
  float *fp;
  fp = &i;
  f(&i);
  char *c = cs;
  int **pp = &fp;
  (void)c; (void)pp;
  return 0;
}
