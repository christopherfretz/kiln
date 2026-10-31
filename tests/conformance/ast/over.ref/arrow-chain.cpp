//@ expect: ast
//@ oracle: clang
// [over.ref]/1: `x->m` is `(x.operator->())->m`, applied until the result
// is a pointer.
struct X { int m; int g(); };
struct P { X *operator->(); };
struct Q { P operator->(); };
struct R { Q &operator->() const; };
int f(P p, Q q, const R &r) { return p->m + q->m + r->g(); }
