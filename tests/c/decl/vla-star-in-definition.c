// C17 6.7.6.2p4: `[*]` (a variable length array of unspecified size) may
// appear only in a declaration that is not a definition (function prototype
// scope); in a function definition's parameters it is an error.
//@ std: c17
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:12 error "variable length array must be bound in function definition"
//@ diag: 10:19 error "variable length array must be bound in function definition"
void f(int a[*]) {}
void g(int n, int a[][*]) {}
void h(int (*p)[*]);
void h2(int a[*][3]);
