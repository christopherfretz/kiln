//@ args: -O1
// Struct-path TBAA at -O1 ([basic.lval]/11, [class.mem]): a member access
// `x.m` / `p->m` is tagged `{base struct node, member type, offset}`, the
// base being the outermost class reached by a chain of `.` from the
// object expression (arrow and subscript start a new path). A class node
// lists its non-virtual non-empty bases, then its fields (a bit-field at
// its byte offset, unnamed bit-fields and empty `[[no_unique_address]]`
// members omitted). A union member, an access through a union, and a
// vector-typed member are "omnipotent char"; a bit-field access is
// untagged; a class with a flexible array member is no valid base (its
// members get scalar tags); a derived-to-base conversion starts a new
// path. A reference member's slot is accessed as its pointer type, its
// referee as the referee's type; a constructor's member initializer
// stores with the member's path.
#define ON __attribute__((optnone))
struct In { short s; int i; };
struct S { int a; long b; In in; char c; float f; double d; int* p; unsigned u; bool bo; int arr[3]; In ina[2]; };
union U { int i; float f; };
struct WU { int x; U u; };
struct Base { int b0; };
struct Der : Base { int d0; };
struct VBase : virtual Base { int z; };
struct BF { int a : 3; int b : 5; int c; };
struct E {};
struct NU { int a; [[no_unique_address]] E e; int b; int : 0; int c : 4; };
struct Fl { int n; int d[]; };
typedef int v4 __attribute__((vector_size(16)));
struct VS { int a; v4 v; };
struct R { int& r; int x; ON R(int& q); };
ON R::R(int& q) : r(q), x(1) {}
struct AU { union { int i; float f; }; int k; ON AU(); };
ON AU::AU() : i(1), k(2) {}
ON long members(S* s, int i) {
  s->a = 1; s->b = 2; s->in.i = 3; s->in.s = 4; s->c = 5; s->f = 6; s->d = 7; s->p = 0;
  s->u = 1; s->bo = true; s->arr[1] = 2; s->arr[i] = 3; s->ina[1].i = 3;
  return s->b + s->in.i;
}
ON int unions(U* u, WU* w) { u->i = 1; u->f = 2; w->u.i = 3; w->x = 4; return u->i + w->u.i; }
ON int derived(Der* d, VBase* v) { d->b0 = 1; d->d0 = 2; static_cast<Base*>(d)->b0 = 3; static_cast<Base&>(*d).b0 = 4; v->z = 1; v->b0 = 2; return d->b0; }
ON int bitfields(BF* b, NU* n) { b->a = 1; b->c = 2; n->a = 1; n->b = 2; n->c = 3; return b->b + n->b; }
ON int flexible(Fl* f) { f->n = 1; f->d[0] = 2; return f->n; }
ON int vector_member(VS* vs, v4* vp) { vs->a = 1; vs->v = *vp; return vs->v[0]; }
ON int ref_member(R* r) { r->r = 2; return r->r + r->x; }
