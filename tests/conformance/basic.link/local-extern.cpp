// [basic.link]/7, [basic.link]/11: block-scope externs and redeclarations with different types
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:12 error "redeclaration of 'm' with a different type: 'int' vs 'long'"
//@ diag: 15:24 note "previous declaration is here"
//@ diag: 18:13 error "redeclaration of 'q' with a different type: 'long' vs 'int'"
//@ diag: 17:12 note "previous declaration is here"
//@ diag: 19:12 warning "'extern' variable has an initializer [-Wextern-initializer]"
//@ diag: 20:13 error "redeclaration of 'r' with a different type: 'long' vs 'int'"
//@ diag: 19:12 note "previous definition is here"
//@ diag: 22:6 error "redefinition of 't' with a different type: 'long' vs 'int'"
//@ diag: 21:12 note "previous declaration is here"
void f() { extern int n; n = 1; }
int n = 0;
void g() { extern long m; }
extern int m;
extern int q;
extern long q;
extern int r = 1;
extern long r;
extern int t;
long t;
