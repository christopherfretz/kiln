// C17 6.5.16.1p1: an integer assigned to a pointer (other than a null
// pointer constant), or a pointer to an integer, violates a constraint.
// clang 19 makes -Wint-conversion an error by default.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:6 error "incompatible integer to pointer conversion initializing 'int *' with an expression of type 'int'"
//@ diag: 15:19 error "incompatible pointer to integer conversion initializing 'int' with an expression of type 'int *'; remove &"
//@ diag: 16:23 error "incompatible integer to pointer conversion returning 'int' from a function with result type 'int *'"
//@ diag: 19:5 error "incompatible pointer to integer conversion passing 'int *' to parameter of type 'int'; remove &"
//@ diag: 17:11 note "passing argument to parameter here"
//@ diag: 20:5 error "incompatible integer to pointer conversion assigning to 'int *' from 'int'"
//@ diag: 21:5 error "incompatible pointer to integer conversion assigning to 'int' from 'int *'; dereference with *"
int *p = 5;
int i;
int k(void) { int j = &i; return j; }
int *f(void) { return 1; }
void g(int);
void h(void) {
  g(&i);
  p = 2;
  i = p;
  p = 0;
  p = (void *)0;
  p = 0L;
}
