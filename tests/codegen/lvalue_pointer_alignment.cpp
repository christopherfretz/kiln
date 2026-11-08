// WP-829: the alignment of an lvalue reached through a pointer follows
// clang `CodeGenFunction::EmitPointerWithAlignment` (spec/codegen.md §3.3
// "Pointer alignment"): `*p` ([expr.unary.op]/1), `p[i]` ([expr.sub]/2),
// `p->m` ([expr.ref]/3), `p->*m` ([expr.mptr.oper]/3), the object of a
// member call through `->`, a vector element through `->`, and the vtable
// pointer load of a virtual derived-to-base conversion ([conv.ptr]/3) and
// of `dynamic_cast` ([expr.dynamic.cast]) take the pointer's known
// alignment: `&lvalue` keeps the lvalue's (`alignas` included), array
// decay the array's, a no-op cast its operand's, and anything else is the
// pointee's natural alignment, which for a class with virtual bases is its
// non-virtual alignment, not its complete-object alignment. A subscript
// is aligned at `index * sizeof(element)` from its base for a constant
// index and at the element size otherwise.

struct alignas(32) S32 { int a; int m; };
struct M64 { int a; alignas(64) int m; };
struct alignas(64) S64 { char c; alignas(32) int m; };

S32 gs32;
M64 gm64;
S64 gs64;

int deref_addr_member() {
  // `*&x.m`: the member's lvalue alignment survives `&` and `*`.
  return *&gs32.m + *&gm64.m + *&gs64.m + *&*&gm64.m;
}

void store_deref_addr_member(int v) {
  *&gs32.a = v;
  *&gm64.m = v;
  (&gs64)->m = v;
  (*&gs64).m = v;
}

int over_aligned_array(int i) {
  alignas(64) int arr[16] = {};
  int *p = arr;
  int r = p[0] + p[3] + p[i];
  // Decay keeps the array's alignment: constant indexes are aligned at
  // their byte offset, a variable index at the element.
  r += (&arr[0])[4] + (+arr)[8] + arr[2] + arr[i];
  int *q = &arr[4];
  return r + q[0] + q[i] + *(arr + 1);
}

struct V { long long v; };
struct alignas(32) VB : virtual V { long long pad; int m; };
struct VD : VB { int d; };
struct alignas(64) WithV : virtual V {
  int m;
  int get() { return m + this->m + (*this).m; }
  void set(int x) { m = x; }
};

// A virtual base stricter than the rest: complete objects are aligned 32,
// but an `NV` base subobject only 8 (its non-virtual alignment).
struct alignas(32) BigV { int v; };
struct NV : virtual BigV {
  long long pad;  // `m` at offset 16: aligned 16 only in a complete object
  int m;
  int get() { return m + this->m + (*this).m; }
};
struct NVD final : NV { int e; };

int nv_member(NV *p, NV &r, NVD *f, int NV::*mp) {
  p->m = 1;
  (*p).m = 2;
  p[0].m = 3;
  // `p->v`: the virtual base is reached at its own non-virtual alignment
  // once `p` is at least `NV`'s non-virtual alignment.
  return p->m + r.m + p->get() + p->*mp + (*p).*mp + f->m + f->e + p->v +
         r.v + f->v;
}

int vbase_member(VB *p, VB &r, WithV *w) {
  // A class with a virtual base is reached at its non-virtual alignment.
  return p->m + r.m + (*p).m + w->m + w->get();
}

long long vbase_reach(VD *d, WithV *w) {
  // The virtual-base conversion loads the vptr at the operand's alignment.
  V *a = d;
  V *b = w;
  return a->v + b->v + d->v + w->v;
}

struct Poly { virtual ~Poly(); int x; };
struct alignas(32) PolyD : Poly { int y; };
void *dyn(PolyD *p) { return dynamic_cast<void *>(static_cast<Poly *>(p)); }

int casts(S32 *s, void *vp, char *cp) {
  // An implicit no-op cast keeps the operand's alignment; an explicit cast
  // of a pointer whose alignment is not a declaration's takes the target's.
  int r = *(int *)vp + *(int *)cp;
  r += *(int *)&gs32 + *(int *)(char *)&gs32;
  r += *(int *)(void *)&gs32.m + ((S32 *)(char *)s)->m;
  r += *reinterpret_cast<int *>(reinterpret_cast<char *>(&gm64) + 64);
  return r + *static_cast<int *>(static_cast<void *>(&gm64.m));
}

struct __attribute__((packed)) Packed { char c; int i; long long l; };
#pragma pack(push, 2)
struct Pack2 { char c; int i; long long l; };
#pragma pack(pop)

Packed gp;
Pack2 gq;

long long packed(Packed *p, Pack2 *q, int Packed::*mp, int Pack2::*mq) {
  long long r = p->i + p->l + q->i + q->l;
  r += (*p).i + gp.l + gq.i;
  r += p->*mp + q->*mq + gp.*mp + gq.*mq;
  p->i = 1;
  q->l = 2;
  return r;
}

int local_array_decay() {
  alignas(32) char buf[64] = {};
  char *p = buf;
  int *ip = (int *)buf;
  buf[1] = 3;
  return p[0] + p[33] + *(buf + 2) + ip[1] + *ip;
}

int assume(int *p) {
  int *a = (int *)__builtin_assume_aligned(p, 32);
  return a[0] + a[1] + *a;
}

typedef int v4i __attribute__((vector_size(16)));
struct alignas(32) VecBox { v4i v; };
int vec_arrow(VecBox *b, v4i *v) { return (&b->v)[0][1] + (*v)[2]; }

struct alignas(16) Obj { int m; int f() const { return m; } };
int member_call(Obj *o, Obj (*arr)[4]) {
  return o->f() + (&gs32)->m + (*arr)[1].f() + (*arr)[1].m;
}
