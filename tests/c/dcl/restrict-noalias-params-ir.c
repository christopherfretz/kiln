// C `restrict` (C23 6.7.4.2; WP-1663): a definition's parameter whose
// declared type is a restrict-qualified pointer is `noalias` (clang
// `EmitFunctionProlog`), including an array parameter declarator with
// `restrict` in its brackets (adjusted to a restrict-qualified pointer,
// 6.7.7.4) and an old-style definition's parameter. A restrict nested
// below the top level is not.
//@ expect: ir
//@ args: -std=gnu17 -Wno-deprecated-non-prototype
void c1(int *restrict a, int b[restrict], int c[restrict 4]) {}
void c2(a, b) int *restrict a; int *b; {}
void c3(int n, int m[restrict][n]) {}
void c4(int *restrict *p, int *q) {}
typedef int *restrict rp;
void c5(rp a) {}
void c6(int *restrict a);
void c6(int *a) {}
