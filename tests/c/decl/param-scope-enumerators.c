// C23 6.2.1p4 (scopes of identifiers): the enumerators of an enumeration
// defined in a parameter list have its function prototype scope, like the
// tag: the rest of the list and a definition's body see them, the scope
// around the declarator does not, so they neither clash with nor hide a
// file-scope or block-scope identifier of the same name (clang
// `ActOnEnumConstant` declares them in the prototype scope).
//@ expect: run
//@ strict-diags
//@ args: -Wno-visibility
//@ stdout: 40 31 4 9 7 8
#include <stdio.h>
enum { Z = 100 };
int X = 40;
static int g(enum E { X = 5, Y } e, int (*a)[Y]) { enum E z = Y; return e + z + (int)sizeof *a; }
static int u(enum { Z = 1, W } e, char (*a)[Z]) { return e + W + Z + (int)sizeof *a; }
static int n(int (*fp)(enum { Q = 3 } q), int *b) { (void)fp; (void)b; return 8; }
int Q = 9;
int main(void) {
  int (*bp)(enum { V = 2 } v);
  int V = 7;
  int a[6];
  char c[1];
  (void)bp;
  printf("%d %d %d %d %d %d\n", X, g(1, &a), u(0, &c), Q, V, n(0, 0));
  return 0;
}
