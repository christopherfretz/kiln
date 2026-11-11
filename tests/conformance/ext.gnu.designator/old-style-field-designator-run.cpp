// GNU old-style field designators in C++ (an extension; [dcl.init.aggr]):
// `name: value` initializes the named member like `.name = value`, for a
// struct, a union member, a braced value and a nested aggregate.
//@ std: c++20
//@ expect: run
//@ stdout: 0 7 8 | 4 | 3 6
#include <cstdio>
struct S { int a, b; int c[2]; };
struct T { S s; int n; };
S w = { b: 7, c: { 8 } };
union U { double d; int i; } u = { i: 4 };
T t = { s: { a: 3, b: 6 }, n: 1 };
int main() {
  std::printf("%d %d %d | %d | %d %d\n", w.a, w.b, w.c[0], u.i, t.s.a, t.s.b);
  return t.n - 1;
}
