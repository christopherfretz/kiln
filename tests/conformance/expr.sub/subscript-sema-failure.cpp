// [expr.sub]/1, [expr.post.general]: a subscript whose semantic check
// fails (no array or pointer operand, a non-integer index, no viable
// operator[]), or whose ']' is missing, is an invalid expression: an
// enclosing call's expression-list skips to its next ',' and diagnoses
// the arguments after it, an expression statement skips to its ';', and
// a declarator's initializer to the next declarator (clang's ExprError).
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 40:22 error "subscripted value is not an array, pointer, or vector"
//@ diag: 40:32 error "invalid operands to binary expression ('const char[2]' and 'int')"
//@ diag: 41:22 error "array subscript is not an integer"
//@ diag: 41:34 error "invalid operands to binary expression ('const char[2]' and 'int')"
//@ diag: 42:22 error "no viable overloaded operator[] for type 'struct S'"
//@ diag: 39:16 note "candidate function not viable: no known conversion from 'const char[2]' to 'int' for 1st argument"
//@ diag: 42:34 error "invalid operands to binary expression ('const char[2]' and 'int')"
//@ diag: 43:22 error "subscripted value is not an array, pointer, or vector"
//@ diag: 43:36 error "invalid operands to binary expression ('const char[2]' and 'int')"
//@ diag: 44:22 error "subscripted value is not an array, pointer, or vector"
//@ diag: 44:36 error "invalid operands to binary expression ('const char[2]' and 'int')"
//@ diag: 45:25 error "subscripted value is not an array, pointer, or vector"
//@ diag: 45:35 error "invalid operands to binary expression ('const char[2]' and 'int')"
//@ diag: 46:14 error "subscripted value is not an array, pointer, or vector"
//@ diag: 46:24 error "invalid operands to binary expression ('const char[2]' and 'int')"
//@ diag: 47:22 error "subscripted value is not an array, pointer, or vector"
//@ diag: 47:36 error "invalid operands to binary expression ('const char[2]' and 'int')"
//@ diag: 48:25 error "expected ']'"
//@ diag: 48:22 note "to match this '['"
//@ diag: 48:34 error "invalid operands to binary expression ('const char[2]' and 'int')"
//@ diag: 49:26 error "expected ')'"
//@ diag: 49:23 note "to match this '('"
//@ diag: 49:35 error "invalid operands to binary expression ('const char[2]' and 'int')"
//@ diag: 49:44 error "invalid operands to binary expression ('const char[2]' and 'int')"
//@ diag: 50:27 error "expected ')'"
//@ diag: 50:21 note "to match this '('"
int f(int, int);
int a[4];
int i;
struct S { int operator[](int); } s;
int c1() { return f(i[1] 5, "a"*2); }
int c2() { return f(a["x"] 6, "b"*2); }
int c3() { return f(s["y"] 7, "c"*2); }
int c4() { return f(i[1] + 1 8, "d"*2); }
int c5() { return f(i[1] = 2 9, "e"*2); }
int c6() { return f(a[0][1] 8, "f"*2); }
void c7() { i[1] 5; "g"*2; }
void c8() { int q = i[1] 5, r = "h"*2; }
int c9() { return f(a[1 2] 3, "i"*2); }
int c10() { return a[f(1 2) 3, "j"*2] + "k"*2; }
int c11() { return f(a[0] 8 [1], "m"*2); }
