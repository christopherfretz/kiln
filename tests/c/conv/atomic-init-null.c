// C23 6.7.11p11, 6.5.17.2p1: the initializer of an atomic object meets
// the constraints of simple assignment to the non-atomic version of its
// type, so `_Atomic(T *)` takes a null pointer constant (`NULL`,
// `(void *)0`, `0`) and `_Atomic(struct S)` a `struct S` value, in braced
// member, union, array element, designated, nested and compound-literal
// initializers, static and automatic, and in assignment and argument
// passing (clang `CheckSingleAssignmentConstraints` on the atomic-
// unqualified type). An `_Atomic(struct S)` operand is used by its value
// (C17 6.3.2.1p2: an atomic load, `AtomicToNonAtomic`) in the same forms.
// Found building an open-source allocator.
//@ args: -std=gnu17
//@ expect: compile-ok
//@ strict-diags
#include <stddef.h>
struct S { int a, b; };
struct s { _Atomic(int *) p; _Atomic(int) i; _Atomic(struct S) st; };
static const struct s v = { NULL, 1 };
static struct s v2 = { .p = (void *)0, .i = 3 };
static struct s v3 = { 0 };
union u { _Atomic(int *) p; int x; };
static union u uu = { NULL };
static _Atomic(int *) arr[2] = { NULL, 0 };
static _Atomic(int *) sc = NULL;
static struct s *cl = &(struct s){ NULL, 4 };
struct nest { struct s in; int k; } nn = { { NULL }, 1 };
struct S gs = { 1, 2 };
void g(_Atomic(int *) p);
int f(void) {
  struct s a = { NULL, 2 };
  _Atomic(int *) q = NULL;
  _Atomic(struct S) as = gs;
  struct S ls = as;
  struct s b = { .st = gs, .p = (void *)0 };
  q = NULL;
  g(NULL);
  g(0);
  return a.i + (q == NULL) + (b.p == 0) + (ls.a == 1) + v3.i + uu.x + (arr[1] == 0)
         + (sc == 0) + cl->i + nn.k + v2.i + (v.p == 0);
}
struct T { struct S s; int c; };
void h(struct S);
struct S r(_Atomic(struct S) *p) { return *p; }
int k(_Atomic(struct S) as) {
  struct S l1 = as;
  struct S l2; l2 = as;
  struct T t = { as, 3 };
  struct T u = { .s = as };
  h(as);
  _Atomic(struct S) a2 = as;
  _Atomic(struct S) a3; a3 = as;
  return l1.a + l2.a + t.c + u.c;
}
