// GNU transparent_union (GCC "Common Type Attributes"; clang
// mergeTransparentUnionType): a parameter of transparent union type is
// compatible with a redeclaration whose parameter has one of the union's
// member types (the composite keeps the union, C23 6.2.7p3 as extended),
// and an argument of a member type converts to the union.
//@ expect: run
//@ xfail(kiln): WP-1286 follow-up, C has no transparent_union: the redeclaration with a member type is "conflicting types" and an argument of a member type does not convert to the union (gcc.c-torture compile/pr46866.c)
//@ stdout: 7 9
int printf(const char *, ...);
typedef union { int *ip; long *lp; } __attribute__((transparent_union)) IP;
int get(IP p);
int get(int *p);
int get(IP p) { return *p.ip; }
int main(void) {
  int i = 7;
  long l = 9;
  printf("%d %d\n", get(&i), get((int *)&l));
  return 0;
}
