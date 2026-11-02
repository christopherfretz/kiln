// [expr.sub]/1, [temp.names]/3: under clang 22 an undeclared template-id
// in a subscript's expression-list is an invalid argument: the list skips
// to its next ',' or ')' (a ']' right after the template-id is skipped
// too), so the subscript reports "expected ']'" where the skip stopped
// and is invalid. A subscript over an operand that merely contains errors
// (a call recovered after a bad argument) stays valid, and its caller
// skips quietly to its ')'.
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 46:23 error "use of undeclared identifier 'u1'"
//@ diag: 46:38 error "invalid operands to binary expression ('const char[2]' and 'int')"
//@ diag: 47:23 error "use of undeclared identifier 'u2'"
//@ diag: 47:38 error "invalid operands to binary expression ('const char[2]' and 'int')"
//@ diag: 47:40 error "expected ']'"
//@ diag: 47:22 note "to match this '['"
//@ diag: 48:23 error "use of undeclared identifier 'u3'"
//@ diag: 48:36 error "invalid operands to binary expression ('const char[2]' and 'int')"
//@ diag: 48:38 error "expected ']'"
//@ diag: 48:22 note "to match this '['"
//@ diag: 49:21 error "use of undeclared identifier 'u4'"
//@ diag: 49:37 error "expected ']'"
//@ diag: 49:20 note "to match this '['"
//@ diag: 50:21 error "use of undeclared identifier 'u5'"
//@ diag: 50:38 error "invalid operands to binary expression ('const char[2]' and 'int')"
//@ diag: 50:40 error "expected ']'"
//@ diag: 50:20 note "to match this '['"
//@ diag: 51:21 error "use of undeclared identifier 'u6'"
//@ diag: 51:35 error "invalid operands to binary expression ('const char[2]' and 'int')"
//@ diag: 52:15 error "use of undeclared identifier 'u7'"
//@ diag: 52:23 error "expected ']'"
//@ diag: 52:14 note "to match this '['"
//@ diag: 52:36 error "invalid operands to binary expression ('const char[2]' and 'int')"
//@ diag: 53:23 error "use of undeclared identifier 'u8'"
//@ diag: 54:25 error "use of undeclared identifier 'u9'"
//@ diag: 55:24 error "use of undeclared identifier 'u10'; did you mean 'c10'?"
//@ diag: 55:5 note "'c10' declared here"
//@ diag: 55:33 error "expected ')'"
//@ diag: 55:23 note "to match this '('"
//@ diag: 55:48 error "invalid operands to binary expression ('const char[2]' and 'int')"
int f(int, int);
int a[4];
int *g(int, int);
int h(int, int);
int c1() { return f(a[u1<int> 3], "a"*2); }
int c2() { return f(a[u2<int>] 3, "b"*2); }
int c3() { return f(a[u3<int>], "c"*2); }
int c4() { return a[u4<int>] + "d"*2; }
int c5() { return a[u5<int>] + 1, "e"*2; }
int c6() { return a[u6<int> 3, "f"*2]; }
void c7() { a[u7<int>]; int z = "g"*2; }
int c8() { return f(g(u8<int> 7, 1)[0] 8, "h"*2); }
int c9() { return f(a[h(u9<int> 7, 1)] 8, "i"*2); }
int c10() { return f(h(u10<int> 7, 1)[0] 8, "j"*2); }
