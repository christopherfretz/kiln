// C89 3.1.1 (C23 6.4.1): inline, restrict, bool, alignas and typeof are
// not keywords of strict C89 (inline and typeof are under -std=gnu89).
//@ std: c89
//@ skip(gcc): GCC has no line comments in C89, and this header is one
//@ expect: run
//@ stdout: 7
#include <stdio.h>
int inline_(int restrict) { return restrict; }
int main(void) {
  int inline = 1, bool = 2, alignas = 3, typeof = 1;
  printf("%d\n", inline_(inline + bool + alignas + typeof));
  return 0;
}
