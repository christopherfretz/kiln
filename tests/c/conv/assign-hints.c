// C17 6.5.16.1p1 constraint violations, with clang's conversion hints
// (`tryToFixConversion`: dereference with *, take the address with &,
// remove * or &) and the type printing of C (`int (void)`, `struct S`).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:8 error "incompatible pointer to integer conversion initializing 'long' with an expression of type 'int *'"
//@ diag: 22:20 error "incompatible pointer to integer conversion initializing 'int' with an expression of type 'int *'; remove &"
//@ diag: 22:34 error "incompatible integer to pointer conversion initializing 'int *' with an expression of type 'int'; remove *"
//@ diag: 22:49 warning "incompatible pointer types initializing 'char *' with an expression of type 'int *'"
//@ diag: 22:63 warning "incompatible pointer types initializing 'int *' with an expression of type 'int **'; dereference with *"
//@ diag: 22:78 warning "incompatible pointer types initializing 'int **' with an expression of type 'int *'; take the address with &"
//@ diag: 23:8 warning "incompatible pointer types initializing 'int *' with an expression of type 'int (*)[3]'"
//@ diag: 23:23 warning "incompatible pointer types initializing 'int (*)[3]' with an expression of type 'int *'"
//@ diag: 23:45 warning "incompatible pointer types initializing 'long *' with an expression of type 'int *'"
//@ diag: 23:62 error "incompatible pointer to integer conversion initializing 'unsigned int' with an expression of type 'int **'"
//@ diag: 24:32 error "incompatible integer to pointer conversion initializing 'int *' with an expression of type 'int'; take the address with &"
//@ diag: 24:47 error "incompatible pointer to integer conversion initializing 'int' with an expression of type 'struct S *'"
//@ diag: 24:64 error "initializing 'double' with an expression of incompatible type 'int *'"
//@ diag: 24:77 error "initializing 'int *' with an expression of incompatible type 'double'"
void h(void) {
  int a[3]; int y = 0; int *q = &y; int **qq = &q;
  long x1 = q; int x2 = &y; int *p1 = *q; char *c1 = &y; int *p2 = qq; int **p3 = q;
  int *p4 = &a; int (*p5)[3] = &a[0]; long *p6 = q; unsigned x3 = qq;
  struct S { int m; } st; int *p7 = st.m; int x4 = &st; double d1 = q; int *p8 = 1.0;
}
