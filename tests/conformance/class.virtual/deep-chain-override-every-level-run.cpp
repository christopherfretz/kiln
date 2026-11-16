// [class.virtual]/2, [class.final.overrider]: a chain of 300 classes, each
// overriding the virtual function of the previous one; the final
// overrider of every subobject is the last class's. Kiln refused chains
// deeper than 256, and replaced every transitively overridden function at
// every level (quadratic per class in the depth).
//@ expect: run
//@ stdout: 300 300 3
#include <cstdio>
struct C { virtual ~C() {} virtual int f() { return 0; } };
#define K(n, b, v) struct n : b { int f() override { return v; } };
#define K10(p, b, v) K(p##0, b, v##0) K(p##1, p##0, v##1) K(p##2, p##1, v##2) \
  K(p##3, p##2, v##3) K(p##4, p##3, v##4) K(p##5, p##4, v##5) K(p##6, p##5, v##6) \
  K(p##7, p##6, v##7) K(p##8, p##7, v##8) K(p##9, p##8, v##9)
#define K100(p, b, v) K10(p##0, b, v##0) K10(p##1, p##09, v##1) K10(p##2, p##19, v##2) \
  K10(p##3, p##29, v##3) K10(p##4, p##39, v##4) K10(p##5, p##49, v##5) \
  K10(p##6, p##59, v##6) K10(p##7, p##69, v##7) K10(p##8, p##79, v##8) \
  K10(p##9, p##89, v##9)
K100(S0, C, 10) K100(S1, S099, 11) K100(S2, S199, 12)
int main() {
  S299 last;
  C *c = &last;
  S100 *mid = &last;
  S002 *low = new S002;
  std::printf("%d %d %d\n", c->f() - 1000 + 1, mid->f() - 1000 + 1, low->f() - 1000 + 1);
  delete low;
}
