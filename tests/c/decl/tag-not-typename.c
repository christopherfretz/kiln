// C17 6.2.3p1, 6.7.2.3: a struct tag is not a type name; using it
// without `struct` is an error.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:1 error "must use 'struct' tag to refer to type 'S'"
//@ diag: 9:16 error "must use 'struct' tag to refer to type 'S'"
struct S { int a; };
S s;
void f(void) { S *p = 0; (void)p; }
