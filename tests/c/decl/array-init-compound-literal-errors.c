// GNU array initialization from a compound literal (clang SK_ArrayInit):
// the literal's array type must be compatible with the target's (C23
// 6.2.7p1), with the target's size unknown or equal, and the literal
// must be constant at file scope or free of side effects.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:20 error "cannot initialize array of type 'int[]' with non-constant array of type 'int[2]'"
//@ diag: 11:71 error "cannot initialize array of type 'int[3]' with array of type 'int[2]'"
//@ diag: 11:98 error "cannot initialize array of type 'long[]' with array of type 'int[1]'"
int g(void);
void f(void) { int b[] = (int[]){g(),2}; int c[2] = (int[]){1,2}; int d[3] = (int[2]){1,2}; long e[] = (int[]){1}; }
