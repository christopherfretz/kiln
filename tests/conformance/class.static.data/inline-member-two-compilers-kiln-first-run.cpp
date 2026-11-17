// inline-member-two-compilers-clang-first-run.cpp with an object of the
// compiler under test linked ahead of clang's (WP-1517, WP-1523).
//@ expect: run
//@ std: c++17
//@ strict-diags
//@ unit: Inputs/inline-member-two-compilers-kiln.cpp
//@ clang-unit: Inputs/inline-member-two-compilers-clang.cpp
//@ stdout: 1 1 1 1
//@ stdout: 7 1 s 42 1
#include "Inputs/inline_member_two_compilers.h"
int count() {
  static int c;
  return ++c;
}
int seven() { return 7; }
int main() {
  Units a = from_clang();
  Units b = from_kiln();
  printf("%d %d %d %d\n", a.n == b.n, a.m == b.m, a.s == b.s, a.v == b.v);
  printf("%d %d %s %d %d\n", *b.n, *b.m, *b.s, *b.v, count() - 1);
}
