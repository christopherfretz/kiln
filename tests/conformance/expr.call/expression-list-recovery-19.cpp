// [expr.call]/1, [expr.post.general]: expression-list-recovery-22.cpp's
// forms under clang 19, which models an undeclared template-id as a delayed
// typo: a valid argument until corrected, so the tokens after it are
// skipped quietly to the ')' (and a call with an invalid argument list is
// an error, not a recovery expression).
//@ requires: clang<22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:21 error "use of undeclared identifier 'u1'"
//@ diag: 25:25 error "use of undeclared identifier 'u2'"
//@ diag: 26:24 error "expected ')'"
//@ diag: 26:21 note "to match this '('"
//@ diag: 27:34 error "expected variable name or 'this' in lambda capture list"
//@ diag: 27:49 error "invalid operands to binary expression ('const char[2]' and 'int')"
//@ diag: 28:21 error "use of undeclared identifier 'u5'"
//@ diag: 29:21 error "use of undeclared identifier 'u6'"
//@ diag: 30:21 error "use of undeclared identifier 'u8'"
//@ diag: 31:21 error "use of undeclared identifier 'u9'"
int f(int, int);
int f3(int, int, int);
struct L { int a, b; };
int h(int, L);
int g(int, int (*)(int));
int c1() { return f(u1<int> 7, "a"*2); }
int c2() { return f3(1, u2<int> 7, "b"*2); }
int c3() { return f3(1 2, 3 4, "c"*2); }
int c4() { return f(u4<int> (1, [2], {3}) 7, "d"*2); }
int c5() { return h(u5<int> 7, {1, "e"*2}); }
int c6() { return g(u6<int> 7, [](int x) { return x + "f"*2; }); }
int c8() { return f(u8<int> 7 {1, 2}, "h"*2); }
int c9() { return f(u9<int> 7 (1, 2) [3, 4], "i"*2); }
