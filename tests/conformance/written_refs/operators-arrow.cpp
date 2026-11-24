// Class member access through `operator->` ([over.ref]/1, [expr.ref];
// spec/ast.md §18.13.2): `x->m` is `(x.operator->())->m`, applied while
// the result has class type, so one `->` token gets one `operator` entry
// per `operator->` called, in the order they apply. A `->` on a pointer
// is built-in and has none.
//@ written-refs: operators-arrow.refs
struct S {
  int m;
  int f() const;
};
struct P1 {
  S *operator->() const;
};
struct P2 {
  P1 operator->() const;
};
struct P3 {
  P2 operator->() const;
};

int use(P1 a, P3 c, S *s) {
  a->m;
  c->m;
  c->f();
  s->m;
  return (*s).m + a.operator->()->m;
}
