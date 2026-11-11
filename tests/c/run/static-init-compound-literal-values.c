// C23 6.5.2.5p5, 6.7.11p4 (GNU C, as clang): a compound literal of scalar,
// structure or vector type in an initializer of an object with static
// storage duration is read as its constant initializer: `(struct X){1, 2}`,
// `(int){7}`, a vector literal `(UV){1, 2, 3, 4}` as an aggregate element,
// and a member of a structure literal.
//@ expect: run
//@ stdout: 1 2 7 | 1 2 3 4 4 3 | 9 | 5 6
#include <stdio.h>
struct X { int a, b; };
struct X x = (struct X){ 1, 2 };
int i = (int){ 7 };
typedef unsigned int UV __attribute__((vector_size(16)));
static UV u[] = { (UV){ 1, 2, 3, 4 }, (UV){ 4, 3, 2, 1 } };
int m = ((struct X){ 8, 9 }).b;
typedef int v2 __attribute__((vector_size(8)));
struct H { v2 v; } h = { (v2){ 5, 6 } };
int main(void) {
  printf("%d %d %d | %u %u %u %u %u %u | %d | %d %d\n", x.a, x.b, i, u[0][0], u[0][1],
         u[0][2], u[0][3], u[1][0], u[1][1], m, h.v[0], h.v[1]);
  return 0;
}
