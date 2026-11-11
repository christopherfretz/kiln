// GNU `transparent_union`: clang sets an IR function's parameter
// attributes once, from the declaration that first needs it
// (`GetOrCreateLLVMFunction`). Here a call through the union-typed first
// declaration of `f` comes before its definition `void f(int *p)`, so
// the definition keeps `ptr` without `noundef`, while a call after the
// definition passes `ptr noundef` (WP-1311; the definition-first case is
// transparent-union-merged-definition-ir.c).
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

void early(int *q) { f(q); }

void f(int *p) { *p = 1; }
void g(U u) { *u.ip = 2; }
void h(U u) { *u.ip = 3; }

void use(int *q) {
  f(q);
  g(q);
  h(q);
  k(q);
}
