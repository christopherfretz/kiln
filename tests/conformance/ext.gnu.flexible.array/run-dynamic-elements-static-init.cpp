// GNU: the elements a static object's initializer gives its flexible
// array member need not be constant: GCC emits the object with room for
// them (past its size when they end there) and stores them during the
// object's dynamic initialization, in order with the other members, for a
// namespace-scope variable, a static local (each time a function's first
// call initializes it) and a union's flexible array member; an element of
// class type is constructed in place. kiln folded the object without them
// (as a constant initialization) and dropped the elements, so the reads
// below went past the emitted object. clang 22 rejects these
// initializers ("cannot compile this flexible array initializer yet");
// GCC 16.1 compiles and runs the same file. Found running GCC's C++
// testsuite. WP-1694.
//@ std: c++20
//@ expect: run
//@ oracle: gcc
//@ skip(clang): clang 22 cannot compile a flexible array initializer that is not constant
//@ args: -Wno-gnu-flexible-array-initializer -Wno-c99-designator -Wno-flexible-array-extensions
//@ stdout: 42 1 7 | 43 0 2 2 | 3 5 | 50 42 42 | 1 2 3 3 3 24 | 9 9 | 10 10 | 8 9
#include <cstdio>
struct A { int a; int b[]; };
struct B { B(); int k; };
struct C { int l; B m[]; };
struct T { long long q; char c; int t[]; };
union U { int a; int b[]; };

int x[4];
int i = 3;
A c = {42, {++x[0], 7}};
A d = {43, {0, ++x[0], ++x[1], 2}};
U u = {.b = {i, 5}};
C n = {50, {B(), B()}};
T t = {1, 2, {i, i, i}};

int once(int v) {
  static A s = {v, {v}};
  return s.a + s.b[0];
}

B::B() : k(42) {}

int main() {
  std::printf("%d %d %d | ", c.a, c.b[0], c.b[1]);
  std::printf("%d %d %d %d | ", d.a, d.b[0], d.b[1], d.b[3]);
  std::printf("%d %d | ", u.b[0], u.b[1]);
  std::printf("%d %d %d | ", n.l, n.m[0].k, n.m[1].k);
  std::printf("%d %d %d %d %d %d | ", int(t.q), t.c, t.t[0], t.t[1], t.t[2],
              int(reinterpret_cast<char *>(&t.t[3]) - reinterpret_cast<char *>(&t)));
  std::printf("%d %d | ", x[0] + 7, x[1] + 8);
  std::printf("%d %d | ", once(5), once(6));
  int j = 8;
  static A e = {j, {j + 1}};
  std::printf("%d %d\n", e.a, e.b[0]);
}
