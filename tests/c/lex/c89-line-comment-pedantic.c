// C89 3.1.9: compiling strict C89, clang takes `//` as a comment except
// when `*` follows it and no line comment came before it in the same file
// (so `4 //**/ 2` in the header is 4 / 2, and after the header's first
// line comment, 8; a line comment in a skipped group counts, silently),
// and -pedantic warns once per file, at the first one.
//@ std: c89
//@ args: -pedantic
//@ skip(gcc): GCC has no line comments in C89, and this header is one
//@ expect: run
//@ strict-diags
//@ diag: 1:1 warning "// comments are not allowed in this language"
//@ stdout: 2 6 8 4 1
#include <stdio.h>
#include "Inputs/c89-first-line-comment.h"
#include "Inputs/c89-skipped-line-comment.h"
int main(void) {
  int b = 1; // a second line comment: no second warning
  printf("%d %d %d %d %d\n", h1, h2, h3, s1, b);
  return 0;
}
