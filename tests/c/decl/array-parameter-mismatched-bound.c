// C17 6.7.6.3p7, 6.7.6.2p4: an array parameter is adjusted to a pointer, so
// redeclaring it with another bound is valid; clang's -Warray-parameter (in
// -Wall, off by default) flags a changed bound. `T[]`, `T *` and `T[*]` have
// no bound; two runtime bounds match unless exactly one is `[*]`, and a `[*]`
// over a variably modified element counts as a runtime bound.
//@ std: c17
//@ args: -Wall
//@ expect: compile-ok
//@ strict-diags
//@ diag: 22:32 warning "argument 'x' of type 'int[3]' with mismatched bound"
//@ diag: 22:13 note "previously declared as 'int[2]' here"
//@ diag: 25:46 warning "argument 'x' of type 'int[*]' with mismatched bound"
//@ diag: 25:20 note "previously declared as 'int[n]' here"
//@ diag: 31:31 warning "argument '' of type 'int[4]' with mismatched bound"
//@ diag: 31:13 note "previously declared as 'int[3]' here"
//@ diag: 32:49 warning "argument 'a' of type 'int[*][3]' with mismatched bound"
//@ diag: 32:20 note "previously declared as 'int[n][3]' here"
//@ diag: 36:46 warning "argument 'a' of type 'int[n]' with mismatched bound"
//@ diag: 36:20 note "previously declared as 'int[*]' here"
void a1(int x); void a1(const int x);
void a2(int *x); void a2(int x[3]);
void a3(int x[2]); void a3(int x[3]);
void a4(int x[]); void a4(int *x);
void a5(int x[*]); void a5(int x[]);
void a6(int n, int x[n]); void a6(int n, int x[*]);
void a7(int n, int x[n]); void a7(int n, int x[n+1]);
void a8(int x[3]); void a8(int x[static 3]);
void a9(int x[3]); void a9(int *x);
typedef int T3[3]; void b1(T3 x); void b1(int x[3]);
void b3(int x[3]); void b3(int x[3]) {}
void b4(int x[3]); void b4(int[4]);
void c1(int n, int a[n][3]); void c1(int n, int a[*][3]);
void c2(int n, int a[3][n]); void c2(int n, int a[3][*]);
void c3(int n, int a[n][n]); void c3(int n, int a[*][n]);
void c4(int n, int a[n][n]); void c4(int n, int a[n][*]);
void c6(int n, int a[*]); void c6(int n, int a[n]);
