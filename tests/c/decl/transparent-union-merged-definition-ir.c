// GNU `transparent_union` with C17 6.2.7p3: a parameter declared as a
// transparent union and redeclared as one of its member types merges to
// the member type (clang `mergeTransparentUnionType`), and clang arranges
// the IR function from the declaration that first needs it (the
// definition here) with that composite type. The definitions of `f`
// (first declared with the union) and `h` (first declared with the
// member) both take `ptr noundef`; `g`, the union throughout, takes
// `ptr` without `noundef`. Before WP-1311 kiln built `f`'s signature
// from its first declaration and dropped `noundef`. A call before the
// definition: transparent-union-called-first-ir.c.
//@ expect: ir
//@ min-oracle: 22
typedef union {
  int *ip;
  const int *cip;
} U __attribute__((transparent_union));

void f(U u);
void g(U u);
void h(int *p);
void k(U);

void f(int *p) { *p = 1; }
void g(U u) { *u.ip = 2; }
void h(U u) { *u.ip = 3; }

void use(int *q) {
  f(q);
  g(q);
  h(q);
  k(q);
}
