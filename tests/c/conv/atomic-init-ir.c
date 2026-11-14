// C23 6.7.11p11: initializing atomic objects from a null pointer constant
// and from non-atomic values, statically (constant initializers) and
// automatically; the IR equals clang's (one `NullToPointer` conversion to
// the atomic pointer type, a C record with an `_Atomic` member laid out
// with no data size apart from its size, atomic accesses at the atomic
// type's alignment), and an `_Atomic(struct S)` operand read by an atomic load
// in initialization, assignment, argument passing and return.
//@ args: -std=gnu17
//@ expect: ir
#include <stddef.h>
struct S { int a, b; };
// No padding: clang 20+ zero-fills padding in C constants, clang 19 does
// not.
struct s { _Atomic(int *) p; _Atomic(int) i, j; _Atomic(struct S) st; };
static const struct s v = { NULL, 1 };
struct s v2 = { .p = (void *)0, .i = 3 };
union u { _Atomic(int *) p; int x; } uu = { NULL };
_Atomic(int *) arr[2] = { NULL, 0 };
struct s *cl = &(struct s){ NULL, 4 };
struct nest { struct s in; int k, k2; } nn = { { NULL }, 1 };
struct S gs = { 1, 2 };
_Atomic(int *) ga = NULL;
void g(_Atomic(int *) p);
int *get(void) { return v.p; }
int f(void) {
  struct s a = { NULL, 2 };
  _Atomic(int *) q = NULL;
  _Atomic(struct S) as = gs;
  struct S ls = as;
  struct s b = { .st = gs, .p = (void *)0 };
  q = NULL;
  ga = q;
  g(NULL);
  return a.i + b.i + (q == NULL) + (ls.a == 1);
}
struct T { struct S s; int c, c2; };
void h(struct S);
struct S r(_Atomic(struct S) *p) { return *p; }
int k(_Atomic(struct S) as) {
  struct S l1 = as;
  struct S l2; l2 = as;
  struct T t = { as, 3 };
  h(as);
  _Atomic(struct S) a2 = as;
  _Atomic(struct S) a3; a3 = as;
  return l1.a + l2.a + t.c + ((struct S)a2).a + ((struct S)a3).b;
}
