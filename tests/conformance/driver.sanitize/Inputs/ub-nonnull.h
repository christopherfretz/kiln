// Calls and returns for the nonnull-attribute, returns-nonnull-attribute
// and builtin checks (clang EmitNonNullArgCheck, EmitReturnBlock,
// EmitCheckedArgForBuiltin; WP-871).
namespace n1 {
__attribute__((nonnull)) void g1(int *p, int *q);
__attribute__((nonnull(2))) void g2(int *p, int *q);
void g3(int *p __attribute__((nonnull)), int n);
__attribute__((returns_nonnull)) int *r1(int *p) { if (p) return p; return p + 1; }
void f(int *p, int *q) { g1(p, q); g2(p, q); g3(p, 1); }
} // namespace n1
namespace n2 {
struct S {
  __attribute__((nonnull(2))) void m(int *p, int *q);
  __attribute__((nonnull)) static void s(int *p, int n);
  S(int *p __attribute__((nonnull)));
  void operator()(int *p __attribute__((nonnull)));
};
__attribute__((nonnull)) void v(const char *f, ...);
void h(int *p __attribute__((nonnull)), int *q __attribute__((nonnull)));
void h(int *p, int *q);
void g(int &r __attribute__((nonnull)));
void f(S *x, int *p, int *q, int &r) {
  x->m(p, q);
  S::s(p, 0);
  S y(p);
  y(q);
  v("a", p, 1);
  h(p, q);
  g(r);
  h(nullptr, &r);
}
} // namespace n2
namespace n3 {
int g;
__attribute__((returns_nonnull)) int *a() { return &g; }
__attribute__((returns_nonnull)) int *b(int *p) { return p; }
__attribute__((returns_nonnull)) int *c(int *p) { __builtin_unreachable(); }
__attribute__((returns_nonnull)) int *d(int *p) { if (p) return p; throw 1; }
struct S {
  __attribute__((returns_nonnull)) int *m(int *p) { return p ? p : &g; }
  __attribute__((returns_nonnull)) virtual int *v(int *p);
};
int *S::v(int *p) { return p; }
__attribute__((returns_nonnull)) int *e(int *p);
int *e(int *p) { while (true) { if (p) return p; } }
int *u() { S s; return s.m(nullptr); }
} // namespace n3
namespace n4 {
int f0(unsigned x) { return __builtin_ctz(x); }
int f1(unsigned long x) { return __builtin_clzl(x); }
int f2(unsigned long long x) { return __builtin_ctzll(x) + __builtin_clzll(x); }
int f3(unsigned x) { return __builtin_clzg(x) + __builtin_ctzg(x, 7); }
int f4(unsigned char x) { return __builtin_clzg(x); }
int f5() { return __builtin_ctz(0); }
} // namespace n4
