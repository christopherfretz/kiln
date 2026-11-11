// GNU transparent_union (GCC "Common Type Attributes"; clang
// mergeFunctionParameterTypes, mergeTransparentUnionType): a parameter of
// transparent union type is compatible with one whose type is compatible
// with a member of the union, in a redeclaration (the composite takes the
// member type) and between function pointer types, and an argument of a
// member type converts to the union; a call built before the
// redeclaration passes the union as its first member (gcc.c-torture
// compile/pr34885.c, compile/pr46866.c).
//@ expect: run
//@ stdout: 7 9 11 1
int printf(const char *, ...);
typedef union { int *ip; long *lp; } __attribute__((transparent_union)) IP;
int get(IP p);
int early(void) { int k = 11; return get(&k); }
int get(int *p);
int get(IP p) { return *p.ip; }
int put(IP p);
int (*fp)(long *) = put;
int put(IP p) { return *p.lp == 9; }
int main(void) {
  int i = 7;
  long l = 9;
  printf("%d %d %d %d\n", get(&i), get((int *)&l), early(), fp(&l));
  return 0;
}
