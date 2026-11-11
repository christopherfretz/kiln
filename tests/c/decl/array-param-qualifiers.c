// C23 6.7.7.4p7 (array declarators): in a parameter declaration the type
// qualifiers and `static` may appear inside the `[ ]` of the outermost array
// derivation; the parameter is adjusted to a qualified pointer. glibc's
// <netdb.h> uses `[__restrict_arr]` (getaddrinfo_a), so every TU that
// includes it fails (c-ares: 69 TUs and the configure probes).
//@ std: c17
//@ expect: run
//@ stdout: 9
#include <stdio.h>
static int sum(int n, const int a[restrict], int b[const 2], int c[static 2]) {
  b[0] = a[0];
  return n + b[0] + c[1];
}
int main(void) {
  int a[1] = {3}, b[2] = {0, 0}, c[2] = {0, 4};
  printf("%d\n", sum(2, a, b, c));
  return 0;
}
