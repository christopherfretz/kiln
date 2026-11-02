// [expr.call]/1, [expr.post.general]: an expression-list argument that
// fails to parse is skipped to the next ',' or the closing ')' (nested
// brackets skipped whole), and the arguments after it are still parsed
// and diagnosed (clang's ParseExpressionList). clang 22 keeps a call whose
// argument list or callee contains errors as a recovery expression, and
// skips quietly to the ')' a valid argument is not followed by.
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 30:21 error "use of undeclared identifier 'u1'"
//@ diag: 30:35 error "invalid operands to binary expression ('const char[2]' and 'int')"
//@ diag: 31:25 error "use of undeclared identifier 'u2'"
//@ diag: 31:39 error "invalid operands to binary expression ('const char[2]' and 'int')"
//@ diag: 32:24 error "expected ')'"
//@ diag: 32:21 note "to match this '('"
//@ diag: 33:34 error "expected variable name or 'this' in lambda capture list"
//@ diag: 34:21 error "use of undeclared identifier 'u5'"
//@ diag: 34:39 error "invalid operands to binary expression ('const char[2]' and 'int')"
//@ diag: 35:21 error "use of undeclared identifier 'u6'"
//@ diag: 35:58 error "invalid operands to binary expression ('const char[2]' and 'int')"
//@ diag: 36:21 error "use of undeclared identifier 'u8'"
//@ diag: 36:42 error "invalid operands to binary expression ('const char[2]' and 'int')"
//@ diag: 37:21 error "use of undeclared identifier 'u9'"
//@ diag: 37:49 error "invalid operands to binary expression ('const char[2]' and 'int')"
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
