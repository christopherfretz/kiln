// [basic.def.odr]/15, [dcl.inline]/6: inline static data members (and,
// from C++17, the redundant redeclaration `constexpr int A::n;`) defined
// in an object of the compiler under test and one of clang link into
// one entity each, whichever object comes first, and each dynamic
// initialization runs once (the guard is shared). Here clang's object
// is linked first (WP-1517, WP-1523).
//@ expect: run
//@ std: c++17
//@ strict-diags
//@ clang-unit: Inputs/inline-member-two-compilers-clang.cpp
//@ stdout: 1 1 1 1
//@ stdout: 7 1 s 42 1
#include "Inputs/inline_member_two_compilers.h"
int count() {
  static int c;
  return ++c;
}
Units from_kiln() { return {&A::n, &A::m, &A::s, &B<int>::v}; }
int seven() { return 7; }
int main() {
  Units a = from_clang();
  Units b = from_kiln();
  printf("%d %d %d %d\n", a.n == b.n, a.m == b.m, a.s == b.s, a.v == b.v);
  printf("%d %d %s %d %d\n", *b.n, *b.m, *b.s, *b.v, count() - 1);
}
