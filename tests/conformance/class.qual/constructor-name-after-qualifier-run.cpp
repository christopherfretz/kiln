// [class.qual]/2: after a nested-name-specifier naming class C, C's own
// name names its constructor, also when a typedef names the class
// (`T0::S0`); after one naming a namespace the same name is an ordinary
// function name (`int N::N()` defines the function N of namespace N),
// as clang's `isCurrentClassName` decides (WP-1745).
//@ expect: run
//@ stdout: ctor 1 2
//@ stdout: N 7
#include <cstdio>

namespace N { int N(); }
int N::N() { return 7; }

typedef struct S0 { S0(); int v; } T0;
T0::S0() : v(1) {}

struct S1 { S1(int); int v; };
using A1 = S1;
A1::S1(int x) : v(x) {}

int main() {
  S0 a;
  S1 b(2);
  std::printf("ctor %d %d\n", a.v, b.v);
  std::printf("N %d\n", N::N());
}
