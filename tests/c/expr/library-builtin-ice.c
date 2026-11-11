// C17 6.6p6 with clang's library builtins: a call of a library function
// clang's evaluator knows (`strlen`, `strcmp`, ...) folds in an integer
// constant expression; outside C++11 there is no "non-constexpr function"
// note (clang `VisitBuiltinCallExpr`, `CheckICE`'s builtin callee). A
// first declaration compatible with the library type (here without a
// prototype) redeclares the implicit library declaration and takes the
// composite type (clang `LazilyCreateBuiltin` on redeclaration lookup,
// `MergeFunctionDecl`), so it is that library function.
//@ std: c17
//@ expect: run
//@ strict-diags
//@ stdout: 3 1 2 8
#include <stdio.h>
unsigned long strlen();
int strcmp(const char *, const char *);
static int sw(int x) {
  switch (x) {
  case strlen("ab"): return 1;
  case strcmp("a", "a") + 5: return 2;
  }
  return 0;
}
_Static_assert(strlen("abc") == 3, "folded");
int main(void) {
  printf("%d %d %d %d\n", (int)strlen("abc"), sw(2), sw(5), (int)sizeof(strlen("")));
  return 0;
}
