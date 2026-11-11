// C17 6.5.16.1p1: assigning between pointers to incompatible types is a
// constraint violation; clang 19 diagnoses it with a warning
// (-Wincompatible-pointer-types) and converts as a cast would. Dropping
// qualifiers warns under -Wincompatible-pointer-types-discards-qualifiers.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 15:7 warning "incompatible pointer types initializing 'long *' with an expression of type 'int *'"
//@ diag: 18:24 warning "returning 'const char *' from a function with result type 'char *' discards qualifiers"
//@ diag: 21:6 warning "incompatible pointer types assigning to 'float *' from 'int *'"
//@ diag: 22:5 warning "incompatible pointer types passing 'int *' to parameter of type 'long *'"
//@ diag: 16:14 note "passing argument to parameter 'p' here"
//@ diag: 23:9 warning "initializing 'char *' with an expression of type 'const char *' discards qualifiers"
//@ diag: 24:9 warning "incompatible pointer types initializing 'int **' with an expression of type 'float **'"
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
