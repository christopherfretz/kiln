// C17 6.5.15p3-6: the type of a conditional expression with pointer, null
// pointer constant and `void` operands. Two pointers to compatible types give
// a pointer to their composite type (6.2.7p3) with both qualifier sets; a
// pointer to `void` gives a pointer to qualified `void`; a null pointer
// constant takes the other operand's type. Incompatible pointer types, or a
// pointer and an integer, violate the constraints: clang warns
// (-Wpointer-type-mismatch: `void *`; -Wconditional-type-mismatch: the
// pointer type) (clang `CheckConditionalOperands`, C). WP-1285.
//@ std: c17
//@ expect: compile-ok
//@ strict-diags
//@ diag: 37:30 warning "pointer type mismatch ('int *' and 'long *')"
//@ diag: 38:30 warning "pointer/integer type mismatch in conditional expression ('int *' and 'int')"
//@ diag: 39:30 warning "pointer/integer type mismatch in conditional expression ('long' and 'int *')"
//@ diag: 43:12 warning "pointer type mismatch ('kfn' (aka 'int (*)(int)') and 'int *')"
#define IS(e, T) _Static_assert(_Generic((e), T: 1, default: 0), #e)
typedef int (*kfn)(int);
struct S { int a; };
void v(void);
void g(int c, kfn k, void *vp, int *ip, long *lp, const int *cip,
       volatile int *vip, const void *cvp, struct S s, int i, long l,
       int (*pa)[], int (*pa3)[3], int (*fnp)(), int (*fp1)(int)) {
  IS(c ? ip : 0, int *);
  IS(c ? 0 : ip, int *);
  IS(c ? (void *)0 : ip, int *);
  IS(c ? ip : vp, void *);
  IS(c ? cip : vp, const void *);
  IS(c ? ip : cvp, const void *);
  IS(c ? cip : vip, const volatile int *);
  IS(c ? ip : cip, const int *);
  IS(c ? k : (void *)0, kfn);
  IS(c ? k : 0, kfn);
  IS(c ? vp : vp, void *);
  IS(c ? pa : pa3, int (*)[3]);
  IS(c ? fnp : fp1, int (*)(int));
  IS(c ? s : s, struct S);
  _Static_assert(_Generic((c ? ip : lp), void *: 1, default: 0), "");
  _Static_assert(_Generic((c ? ip : i), int *: 1, default: 0), "");
  _Static_assert(_Generic((c ? l : ip), int *: 1, default: 0), "");
  IS(c ? i : 2.0, double);
  IS(c ? 'a' : (short)1, int);
  c ? v() : (void)0;
  (void)(c ? k : ip);
}
