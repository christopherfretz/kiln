// GNU `__extension__` silences only extension diagnostics, only inside
// the declaration it prefixes: an ordinary error in it is still reported,
// and the next declaration, member or statement is checked again (GCC
// 16.1, clang 22). Found running GCC's C++ testsuite (WP-1466).
//@ expect: compile-fail
//@ std: c++98
//@ args: -pedantic-errors
//@ strict-diags
//@ diag: 13:19 error "cannot initialize a variable of type 'int' with an lvalue of type 'const char[2]'"
//@ diag: 14:12 error "zero size arrays are an extension"
//@ diag: 15:43 error "zero size arrays are an extension"
//@ diag: 16:43 error "zero size arrays are an extension"
__extension__ int bad = "x";
char after[0];
struct T { __extension__ int m[0]; char n[0]; };
int k() { __extension__ char q[0]; char r[0]; return (int)sizeof(q) + (int)sizeof(r) + (__extension__ sizeof(char[0])); }
