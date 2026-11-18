// GNU mode; C17 6.7.6.3p7 (C23 6.7.7.4p7; WP-1668): a parameter declared with
// `static` in its outermost array derivation points to the first element
// of an array of at least that many elements, so a definition's
// parameter is `align` of the element type with `dereferenceable(N *
// sizeof element)` when the bound is a nonzero constant and the element
// has a constant size, else `nonnull` (clang `EmitFunctionProlog`): with
// and without `restrict` or other qualifiers in the brackets, a variable
// bound, a variably modified element, a zero bound. The declaration a
// definition redeclares does not count, only the definition's own
// parameter does; a parameter without `static` gets nothing.
//@ expect: ir
//@ args: -std=gnu17
typedef struct { char c[3]; } T3;
void f1(int d[static restrict 2]) { d[0] = 1; }
void f2(int d[static 2]) { d[0] = 1; }
void f3(int n, int d[static n]) { d[0] = n; }
void f4(int d[static 1], double e[static 3], T3 t[static 2]) { d[0] = 1; }
void f5(int d[restrict 2], int e[2], int g[static const 4]) { d[0] = 1; }
void f6(int d[static 0]) {}
void f7(int n, int d[static n][4]) {}
void f8(int d[static 2][3]) {}
void f9(int n, int d[static 2][n]) {}
void f10(char c[static 7], short s[static 5]) {}
void f11(int d[static 2]);
void f11(int d[]) { d[0] = 0; }
void f12(int d[]);
void f12(int d[static 5]) { d[0] = 0; }
void f13(int d[static 2]);
void use(void) {
  int a[2];
  f13(a);
}
