// [expr.call]/1: expression-list-semi-22.cpp's forms under clang 19,
// whose expression-list skip after an invalid argument goes past a ';'
// to the ')': what follows the ';' inside the call is skipped unseen.
//@ requires: clang<22
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 16:19 error "expected ')'"
//@ diag: 16:16 note "to match this '('"
//@ diag: 17:16 error "subscripted value is not an array, pointer, or vector"
//@ diag: 18:19 error "expected ']'"
//@ diag: 18:16 note "to match this '['"
int f(int, int);
int a[4];
int i;
void c1() { f(f(1 2) 7; "a"*2); }
void c2() { f(i[1] 7; "b"*2); }
void c3() { f(a[1 2] 7; "c"*2); }
