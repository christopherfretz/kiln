// [temp.names]/3: clang 19's delayed typo correction of an undeclared
// template-id (spec/sema.md "Undeclared template-ids"): a statement
// keyword alone at the least distance (break, continue, return) replaces
// the name and is re-parsed ("expected unqualified-id"); two values at the
// least distance are no correction; a static data member is corrected
// qualified; a member function or function template is no correction.
//@ requires: clang<22
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 34:19 error "use of undeclared identifier 'valc'"
//@ diag: 35:19 error "use of undeclared identifier 'valeu'"
//@ diag: 36:24 error "use of undeclared identifier 'breakk'; did you mean 'break'?"
//@ diag: 36:24 error "expected unqualified-id"
//@ diag: 37:19 error "use of undeclared identifier 'f'"
//@ diag: 38:19 error "use of undeclared identifier 'sv'; did you mean 'A::sv'?"
//@ diag: 32:54 note "'A::sv' declared here"
//@ diag: 39:19 error "use of undeclared identifier 'g'"
//@ diag: 40:19 error "use of undeclared identifier 'tff'"
//@ diag: 41:19 error "use of undeclared identifier 'valc'"
//@ diag: 42:12 error "use of undeclared identifier 'valc'"
//@ diag: 43:20 error "use of undeclared identifier 'valc'"
//@ diag: 44:17 error "use of undeclared identifier 'valc'"
//@ diag: 45:37 error "use of undeclared identifier 'mn'"
//@ diag: 46:28 error "use of undeclared identifier 'contine'; did you mean 'continue'?"
//@ diag: 46:28 error "expected unqualified-id"
//@ diag: 47:19 error "use of undeclared identifier 'retrn'; did you mean 'return'?"
//@ diag: 47:19 error "expected unqualified-id"
int vala;
int vale(int);
int valx;
struct A { static int f(int); int g(int); static int sv; };
template<class T> int tf(T);
int c1() { return valc<int> + 1; }
int c2() { return valeu<int> + 1; }
void c3() { for (;;) { breakk<int>; } }
int c4() { return f<int> + 1; }
int c5() { return sv<int> + 1; }
int c6() { return g<int> + 1; }
int c7() { return tff<int> + 1; }
int c8() { return valc<int>; }
int c9() { valc<int>; return 0; }
int d1() { int q = valc<int>; return q; }
void d2() { if (valc<int>) {} }
struct B { int mm; void h() { (void)mn<int>; } };
void k4() { while (true) { contine<int>; } }
int k5() { return retrn<int>; }
