// GNU cast to a union type (GCC "Cast to a Union Type"; clang
// `CheckCStyleCast`, `CK_ToUnion`): `(union U)x` where `x` has the
// unqualified type of a member is a `union U` value with that member
// active: the first matching member, a later member too, a structure
// member, a pointer member; the operand a constant, an lvalue, a `const`
// and a `volatile` lvalue; in a file-scope static initializer (a constant,
// as in clang and GCC), nested in a structure initializer and a compound
// literal, as an argument, through a typedef. A local static initialized
// from a cast of a compound literal is a constant for clang (kiln follows
// it); GCC 16.1 rejects that one ("initializer element is not constant"). It was rejected as "no matching conversion
// for C-style cast" since WP-1470 closed the C++20 parenthesized aggregate
// initialization that accepted it by accident (first member only).
// Reduced from GCC's C torture `execute/960416-1.c`. WP-1587.
//@ std: c17
//@ expect: run
//@ strict-diags
//@ stdout: 1 7 2.5 3 4 hi 5 6 9 10 11 12 13 14 0
#include <stdio.h>
struct P { short a, b; };
union U { int i; double d; struct P p; const char *s; long long ll; };
typedef union { unsigned long long d; struct { unsigned h, l; } s; } W;
static union U g1 = (union U)1;
static union U g2 = (union U)2.5;
static const char msg[] = "hi";
static union U g3 = (union U)(const char *)msg;
struct Box { int tag; union U u; };
static struct Box gb = { 4, (union U)5LL };
static int pick(union U u, int which) {
  return which ? (int)u.ll : u.i;
}
int main(void) {
  struct P p = { 7, 8 };
  union U a = (union U)p;
  const int ci = 3;
  volatile int vi = 4;
  static union U ls = (union U)(struct P){ 6, 0 };
  struct Box b = { 9, (union U)10 };
  union U *cp = &(union U){ 0 };
  *cp = (union U)11;
  W w = (W)12ULL;
  long long ll = 14;
  printf("%d %d %.1f %d %d %s %lld %d %d %d %d %d %d %d %d\n", g1.i, a.p.a, g2.d,
         ((union U)ci).i, ((union U)vi).i, g3.s, gb.u.ll, ls.p.a, b.tag, b.u.i,
         cp->i, (int)w.d, pick((union U)13, 0), pick((union U)ll, 1),
         pick((union U)0, 0));
  return 0;
}
