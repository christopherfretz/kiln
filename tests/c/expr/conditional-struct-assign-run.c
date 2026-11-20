// C17 6.5.16/3: a structure assignment is a prvalue of the structure
// type, usable as an initializer, an argument, the operand of `.` and an
// arm of `?:` (WP-1739: the last was rejected by code generation; clang
// `AggExprEmitter::VisitBinAssign`). Also a union cast (GNU) and an
// atomic structure read.
//@ expect: run
//@ stdout: 3 4 4 3 4 3 9 9 9 main 9
#include <stdio.h>
struct S { int a, b; };
union U { int i; float f; struct S s; };
struct S s1 = {1, 2}, s2 = {3, 4}, s3;
_Atomic struct S as;
struct S id(struct S s) { return s; }
int main(void) {
  struct S t = (s1 = s2);
  struct S w = id(s3 = s1);
  int m = (s3 = s2).b;
  union U u = (union U)3;
  union U u2 = (union U)s1;
  as = s2;
  struct S x = as;
  struct S y = (s1 = s2 = (struct S){9, 10});
  char buf[] = "main";
  struct S z = 1 ? (s3 = s2) : s1;
  printf("%d %d %d %d %d %d %d %d %d %s %d\n", t.a, w.b, m, u.i, u2.s.b, x.a, y.a, s2.a, s3.a,
         buf, z.a);
  return 0;
}
