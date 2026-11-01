//@ strict-diags
//@ diag: 58:13 warning "taking address of packed member 'i' of class or structure 'P' may result in an unaligned pointer value"
//@ diag: 60:13 warning "taking address of packed member 'i' of class or structure 'P' may result in an unaligned pointer value"
//@ diag: 61:13 warning "taking address of packed member 'i' of class or structure 'P' may result in an unaligned pointer value"
//@ diag: 65:24 warning "taking address of packed member 'i' of class or structure 'P' may result in an unaligned pointer value"
//@ diag: 66:8 warning "taking address of packed member 'i' of class or structure 'P' may result in an unaligned pointer value"
//@ diag: 69:32 warning "taking address of packed member 'i' of class or structure 'P' may result in an unaligned pointer value"
//@ diag: 72:14 warning "taking address of packed member 'i' of class or structure 'P' may result in an unaligned pointer value"
//@ diag: 73:13 warning "taking address of packed member 's' of class or structure 'P' may result in an unaligned pointer value"
//@ diag: 74:20 warning "taking address of packed member 'i' of class or structure 'P' may result in an unaligned pointer value"
//@ diag: 75:10 warning "taking address of packed member 'i' of class or structure 'P' may result in an unaligned pointer value"
//@ diag: 76:14 warning "taking address of packed member 'i' of class or structure 'P' may result in an unaligned pointer value"
//@ diag: 77:15 warning "taking address of packed member 'i' of class or structure 'P' may result in an unaligned pointer value"
//@ diag: 84:13 warning "taking address of packed member 'i' of class or structure 'Q' may result in an unaligned pointer value"
//@ diag: 86:13 warning "taking address of packed member 'i' of class or structure 'P' may result in an unaligned pointer value"
//@ diag: 88:13 warning "taking address of packed member 'i' of class or structure 'Q' may result in an unaligned pointer value"
//@ diag: 90:13 warning "taking address of packed member 'i' of class or structure 'T' may result in an unaligned pointer value"
//@ diag: 92:14 warning "taking address of packed member 'i' of class or structure 'P' may result in an unaligned pointer value"
//@ diag: 101:14 warning "taking address of packed member 'i' of class or structure 'P' may result in an unaligned pointer value"
//@ diag: 107:13 warning "taking address of packed member 'i' of class or structure 'A' may result in an unaligned pointer value"
//@ diag: 110:14 warning "taking address of packed member 'l' of class or structure 'A' may result in an unaligned pointer value"
//@ diag: 119:8 warning "taking address of packed member 'i' of class or structure 'B' may result in an unaligned pointer value"
//@ diag: 121:13 warning "taking address of packed member 'i' of class or structure 'B' may result in an unaligned pointer value"
//@ diag: 125:17 warning "taking address of packed member 'i' of class or structure 'B' may result in an unaligned pointer value"
//@ diag: 127:14 warning "taking address of packed member 'l' of class or structure 'B' may result in an unaligned pointer value"
//@ diag: 133:13 warning "taking address of packed member 'i' of class or structure 'M' may result in an unaligned pointer value"
//@ diag: 134:12 warning "taking address of packed member 'i' of class or structure 'M' may result in an unaligned pointer value"
//@ diag: 135:24 warning "taking address of packed member 'i' of class or structure 'M' may result in an unaligned pointer value"
//@ diag: 136:25 warning "taking address of packed member 'i' of class or structure 'M' may result in an unaligned pointer value"
//@ diag: 139:38 warning "taking address of packed member 'i' of class or structure 'P' may result in an unaligned pointer value"
//@ diag: 142:12 warning "taking address of packed member 'i' of class or structure 'P' may result in an unaligned pointer value"
//@ diag: 143:41 warning "taking address of packed member 'i' of class or structure 'P' may result in an unaligned pointer value"
//@ diag: 145:44 warning "taking address of packed member 'i' of class or structure 'P' may result in an unaligned pointer value"
//@ diag: 146:24 note "in instantiation of function template specialization 'tg<P>' requested here"
//@ diag: 147:57 warning "taking address of packed member 'i' of class or structure 'P' may result in an unaligned pointer value"
//@ diag: 148:29 note "in instantiation of member function 'W<P>::get' requested here"
// -Waddress-of-packed-member (WP-183): clang's RefersToMemberWithReducedAlignment.
// Taking the address of a member whose alignment `packed` reduced warns at
// the end of the full-expression unless the address is converted to an
// integer, to a pointer to an incomplete type or to a pointer whose pointee
// needs no more alignment than the member has.
struct __attribute__((packed)) P { char c; int i; short s; char d; };
struct Q { char c; int i __attribute__((packed)); };
struct O { char c; P p; };
struct __attribute__((packed)) R { char c; Q q; };
typedef struct __attribute__((packed)) { char c; int i; } T;
struct __attribute__((packed, aligned(4))) B { char c; int i; long l; };
struct __attribute__((packed)) A { int i; long l; };
struct D : P { int j; };
struct S { int *q; void *v; };
struct C { C(int *); };
P &getp();
void use(int *);
void usev(void *);

void basic(P &pr, P *pp) {
  P p;
  int *a = &p.i;              // warns at `p`, the member access's start
  char *b = &p.c;             // alignment 1: no warning
  int *c = &pr.i;
  int *d = &pp->i;
  void *e = &p.i;             // converted to void*: discarded
  char *g = (char *)&p.i;
  long h = (long)&p.i;
  short *k = (short *)&p.i;
  use(&p.i);
  usev(&p.i);
  int &ref = p.i;             // no address taken
  int *v = static_cast<int *>(&p.i);
  void *w = static_cast<void *>(&p.i);
  char *x = reinterpret_cast<char *>(&p.i);
  int *y = &(p.i);
  auto z = &p.s;
  const int *ci = &p.i;
  (void)&p.i;
  int *pa = &p.i + 1;
  bool cmp = &p.i == ci;
  (void)a; (void)b; (void)c; (void)d; (void)e; (void)g; (void)h; (void)k;
  (void)ref; (void)v; (void)w; (void)x; (void)y; (void)z; (void)pa; (void)cmp;
}

void chains() {
  Q q;
  int *m = &q.i;              // packed member: names Q
  O o;
  int *n = &o.p.i;            // names P
  R r;
  int *s = &r.q.i;            // the innermost packed member: Q's `i`
  T t;
  int *u = &t.i;              // an anonymous class is named by its typedef
  D dd;
  int *dv = &dd.i;            // through the derived-to-base conversion
  (void)m; (void)n; (void)s; (void)u; (void)dv;
}

void bases(P *pa, D *dp, bool f, P &x, P &y) {
  int *a1 = &getp().i;        // not a named object or `this`: no warning
  int *a2 = &(*pa).i;
  int *a3 = &pa[1].i;
  int *a4 = &(f ? x : y).i;
  int *a5 = &dp->i;
  (void)a1; (void)a2; (void)a3; (void)a4; (void)a5;
}

void aligned() {
  A a;
  int *x = &a.i;              // offset 0, but A is 1-aligned
  alignas(4) A a2;
  int *y = &a2.i;             // the variable's alignment suffices
  long *z = &a2.l;
  (void)x; (void)y; (void)z;
}

// B is 4-aligned, so `i` is 4-aligned as a member: an initialization of an
// int* discards it, other uses of the address do not.
int *bret(B &b) { return &b.i; }
void binit(B &b, int *q, bool f) {
  int *x;
  x = &b.i;
  bool e = &b.i == q;
  int *y = &b.i + 1;
  use(&b.i);
  S s{&b.i, &b.i};
  C c(&b.i);
  int *t = f ? &b.i : q;
  int *w(&b.i);
  long *u = &b.l;
  (void)x; (void)e; (void)y; (void)s; (void)c; (void)t; (void)w; (void)u;
}

struct __attribute__((packed)) M {
  char c; int i;
  int *m = &i;                // default member initializer
  M() : m(&i) {}
  int *get() { return &i; }   // implicit `this`: at the member name
  int *get2() { return &this->i; }
};

constexpr int k() { P p{}; int *q = &p.i; return q == nullptr; }
static_assert(k() == 0);
P gp;
int *gq = &gp.i;
void lam(P &p) { auto l = [&] { return &p.i; }; (void)l; }

template <class X> int *tg(X &x) { return &x.i; }
int *ti(P &p) { return tg(p); }
template <class U> struct W { U u; int *get() { return &u.i; } };
int *tw(W<P> &w) { return w.get(); }
