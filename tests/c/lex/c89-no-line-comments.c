// C89 3.1.9 (C99 6.4.9 added line comments): strict C89 has no `//`
// comments, but clang lexes `//` as one when compiling C89, even in
// `4 //**/ 2`, which C89 reads as `4 / 2` (c89-no-line-comments-pp.c is
// the preprocessed-output case).
//@ std: c89
//@ skip(gcc): GCC has no line comments in C89, and this header is one
//@ expect: run
//@ stdout: 4 1
#include <stdio.h>
int main(void) {
  int a = 4 //**/ 2
  ;
  int b = 1; // an ordinary comment still works
  printf("%d %d\n", a, b);
  return 0;
}
