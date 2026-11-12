// C23 6.5.4.3p1: unary `+` and `-` want an arithmetic operand, `~` an
// integer one, `!` a scalar one. An array or function operand is first
// converted to a pointer (6.3.2.1p3-p4), so `+a` is an error reported
// with the pointer type (as clang), and `!a` is valid (`sizeof(!a)` is
// `sizeof(int)`).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:17 error "invalid argument type 'int *' to unary expression"
//@ diag: 17:17 error "invalid argument type 'int *' to unary expression"
//@ diag: 18:17 error "invalid argument type 'int *' to unary expression"
//@ diag: 20:17 error "invalid argument type 'void (*)(void)' to unary expression"
//@ diag: 21:17 error "invalid argument type 'void (*)(void)' to unary expression"
//@ diag: 22:17 error "invalid argument type 'void (*)(void)' to unary expression"
int a[10];
void fn(void);
int s1 = sizeof(+a);
int s2 = sizeof(-a);
int s3 = sizeof(~a);
_Static_assert(sizeof(!a) == sizeof(int), "");
int s5 = sizeof(+fn);
int s6 = sizeof(-fn);
int s7 = sizeof(~fn);
_Static_assert(sizeof(!fn) == sizeof(int), "");
