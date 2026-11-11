// C23 6.2.1p4 (scopes of identifiers): a structure defined in a parameter
// list is in scope in the function body and hides the file-scope tag of
// the same name only there.
//@ expect: run
//@ args: -Wno-visibility
//@ stdout: 7 11 4 9
#include <stdio.h>
struct K { int a; };
static int f(struct K { int b, c; } k) { struct K w = k; return w.b + w.c; }
static int g(enum E { X = 5, Y } e) { enum E z = Y; return e + z; }
static int h(struct T { int v; } *a, struct T *b) { return a->v + b->v; }
struct R { int r; } mk(int v) { struct R x = { v }; return x; }
struct K kv = { 7 };
int main(void) {
  printf("%d %d %zu %d\n", kv.a, g(5), sizeof kv, mk(9).r);
  (void)f;
  (void)h;
  return 0;
}
