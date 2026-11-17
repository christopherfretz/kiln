// The same redefinitions outside a GNU dialect (-std=c++NN): clang's
// plain "redefinition" (extern-inline-redefinition-gnu.cpp has the GNU
// dialect wording). `gnu_inline` without `extern` on a member function
// warns, and a later definition is a redefinition in every dialect; on a
// declaration that is not inline the attribute is ignored and the
// definition stands. WP-1550.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:5 error "redefinition of 'f'"
//@ diag: 18:47 note "previous definition is here"
//@ diag: 20:27 warning "'gnu_inline' attribute without 'extern' in C++ treated as externally available, this changed in Clang 10"
//@ diag: 21:8 error "redefinition of 'm'"
//@ diag: 20:51 note "previous definition is here"
//@ diag: 22:34 warning "'gnu_inline' attribute requires function to be marked 'inline', attribute ignored"
//@ diag: 24:5 error "redefinition of 'k'"
//@ diag: 23:19 note "previous definition is here"
int e(int x) { return x; }
extern inline __attribute__((gnu_inline)) int f(int x) { return 2; }
int f(int x) { return 1; }
struct S { __attribute__((gnu_inline)) inline int m(int x) { return 2; } };
int S::m(int x) { return 1; }
extern int d(int) __attribute__((__gnu_inline__));
extern inline int k(int x) { return 2; }
int k(int x) { return 1; }
int d(int x) { return e(x); }
