// [over.ref]/1: `x->m` is `(x.operator->())->m`, applied until a pointer
// results; no operator->, a cycle, deleted and inaccessible ones.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:23 error "member reference type 'S' is not a pointer; did you mean to use '.'?"
//@ diag: 29:25 error "member reference type 'int' is not a pointer"
//@ diag: 31:24 error "circular pointer delegation detected"
//@ diag: 30:59 note "'operator->' declared here produces an object of type 'C2'"
//@ diag: 30:27 note "'operator->' declared here produces an object of type 'C1'"
//@ diag: 34:23 error "member reference type 'N' is not a pointer"
//@ diag: 33:12 note "'->' applied to return value of the operator->() declared here"
//@ diag: 36:23 error "overload resolution selected deleted operator '->'"
//@ diag: 35:15 note "candidate function has been explicitly deleted"
//@ diag: 38:23 error "no viable overloaded 'operator->'"
//@ diag: 37:15 note "candidate function not viable: expects an rvalue for object argument"
//@ diag: 40:25 error "'operator->' is a private member of 'Pr'"
//@ diag: 39:25 note "declared private here"
//@ diag: 41:30 error "member reference type 'int' is not a pointer"
struct X { int m; int f(); };
struct P { X* operator->(); };
struct Q { P operator->(); };
struct R { Q& operator->() const; };
int a1(P p) { return p->m; }
int a2(Q q) { return q->m + q->f(); }
int a3(const R& r) { return r->m; }
struct S { int m; };
int a4(S s) { return s->m; }
struct I { int operator->(); };
int a5(I i) { return i->m; }
struct C1; struct C2 { C1 operator->(); }; struct C1 { C2 operator->(); };
int a6(C1 c) { return c->m; }
struct N { };
struct M { N operator->(); };
int a7(M m) { return m->x; }
struct D { X* operator->() = delete; };
int a8(D d) { return d->m; }
struct V { X* operator->() &&; };
int a9(V v) { return v->m; }
struct Pr { private: X* operator->(); };
int a11(Pr p) { return p->m; }
int x; int a12() { return x->m; }
