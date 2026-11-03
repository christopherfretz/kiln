// [temp.names]/3: clang 19's delayed typo correction of an undeclared
// template-id (spec/sema.md "Undeclared template-ids") by correction
// point: a built-in operand or an initializer takes the nearest value over
// an (invalid) function at the same distance; a return, condition or
// discarded expression checks only the rebuilt name, so the two tie and
// nothing is corrected; nor is a callee.
//@ requires: clang<22
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 27:19 error "use of undeclared identifier 'valc'; did you mean 'vala'?"
//@ diag: 22:5 note "'vala' declared here"
//@ diag: 28:19 error "use of undeclared identifier 'valeu'"
//@ diag: 29:19 error "use of undeclared identifier 'valy'"
//@ diag: 30:19 error "use of undeclared identifier 'valc'"
//@ diag: 31:12 error "use of undeclared identifier 'valc'"
//@ diag: 32:20 error "use of undeclared identifier 'valc'; did you mean 'vala'?"
//@ diag: 22:5 note "'vala' declared here"
//@ diag: 33:17 error "use of undeclared identifier 'valc'"
//@ diag: 34:20 error "use of undeclared identifier 'valc'"
//@ diag: 35:19 error "use of undeclared identifier 'valc'"
int vala;
int vale(int);
int valyy;
struct A { static int f(int); int g(int); static int sv; };
template<class T> int tf(T);
int c1() { return valc<int> + 1; }
int c2() { return valeu<int> + 1; }
int c3() { return valy<int> + 1; }
int c8() { return valc<int>; }
int c9() { valc<int>; return 0; }
int d1() { int q = valc<int>; return q; }
void d2() { if (valc<int>) {} }
int d3() { return (valc<int>); }
int d4() { return valc<int>(1); }
