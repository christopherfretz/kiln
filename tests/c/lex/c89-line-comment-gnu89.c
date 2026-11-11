// GNU C89 (clang -std=gnu89) has line comments: no warning under
// -pedantic, and `4 //**/ 2` is 4 even as the first comment of a file.
//@ args: -std=gnu89 -pedantic
//@ skip(gcc): -pedantic GCC warns about the header's line comments
//@ expect: run
//@ strict-diags
//@ stdout: 4 6 8
#include <stdio.h>
#include "Inputs/c89-first-line-comment.h"
int main(void) {
  printf("%d %d %d\n", h1, h2, h3);
  return 0;
}
