// [expr.call]/1, [stmt.expr]/1: clang 22's expression-list skip after an
// invalid argument stops at a ';' (clang 19's goes past it). The call
// ends there; the expression statement after the ';' is parsed and
// diagnosed, and its stray ')' before ';' is reported and taken.
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 23:19 error "expected ')'"
//@ diag: 23:16 note "to match this '('"
//@ diag: 23:28 error "invalid operands to binary expression ('const char[2]' and 'int')"
//@ diag: 23:30 error "extraneous ')' before ';'"
//@ diag: 24:16 error "subscripted value is not an array, pointer, or vector"
//@ diag: 24:26 error "invalid operands to binary expression ('const char[2]' and 'int')"
//@ diag: 24:28 error "extraneous ')' before ';'"
//@ diag: 25:19 error "expected ']'"
//@ diag: 25:16 note "to match this '['"
//@ diag: 25:28 error "invalid operands to binary expression ('const char[2]' and 'int')"
//@ diag: 25:30 error "extraneous ')' before ';'"
int f(int, int);
int a[4];
int i;
void c1() { f(f(1 2) 7; "a"*2); }
void c2() { f(i[1] 7; "b"*2); }
void c3() { f(a[1 2] 7; "c"*2); }
