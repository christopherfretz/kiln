// Conversions between address spaces that clang accepts in C++ (spec/sema.md
// "Address spaces"; WP-1803): a pointer converts implicitly only into a
// superset of its pointee's space (generic holds every space but constant,
// global holds global_device and global_host), with added cv; any
// pointer converts with a C-style cast; a qualification conversion may
// change the space at the first pointee level only. The negative twins are
// address-space-conversion-errors.cpp.
//@ expect: compile-ok
//@ strict-diags

void same(__attribute__((address_space(1))) int *p) {
  __attribute__((address_space(1))) int *q = p;
  __attribute__((address_space(1))) const int *c = p;
  __attribute__((address_space(1))) const volatile int *cv = p;
  __attribute__((address_space(1))) void *v = p;
  const __attribute__((address_space(1))) void *cvv = p;
  q = c == p ? q : nullptr;
}
void casts(__attribute__((address_space(1))) int *p, int *d) {
  int *a = (int *)p;
  __attribute__((address_space(2))) int *b = (__attribute__((address_space(2))) int *)p;
  __attribute__((address_space(1))) int *c = (__attribute__((address_space(1))) int *)d;
  __attribute__((address_space(1))) char *e = (__attribute__((address_space(1))) char *)p;
  long i = (long)p;
  __attribute__((address_space(1))) int *f = (__attribute__((address_space(1))) int *)i;
  __attribute__((address_space(1))) int *n = nullptr;
  __attribute__((address_space(1))) int *z = 0;
  (void)a; (void)b; (void)c; (void)e; (void)f; (void)n; (void)z;
}
void generic(__attribute__((opencl_global)) int *g,
             __attribute__((opencl_local)) int *l,
             __attribute__((opencl_private)) int *p, int *d) {
  __attribute__((opencl_generic)) int *a = g;
  __attribute__((opencl_generic)) int *b = l;
  __attribute__((opencl_generic)) int *c = p;
  __attribute__((opencl_generic)) int *e = d;
  __attribute__((opencl_generic)) const int *f = g;
  __attribute__((opencl_generic)) void *v = g;
  __attribute__((opencl_global)) int *back = (__attribute__((opencl_global)) int *)a;
  (void)b; (void)c; (void)e; (void)f; (void)v; (void)back;
}
void global(__attribute__((opencl_global_device)) int *dv,
            __attribute__((opencl_global_host)) int *h) {
  __attribute__((opencl_global)) int *q = dv;
  q = h;
}
bool compare(__attribute__((opencl_generic)) int *g, __attribute__((opencl_global)) int *q,
             __attribute__((opencl_generic)) int *g2, int *d,
             __attribute__((address_space(1))) int *a, __attribute__((address_space(1))) const int *b) {
  return g == q && g2 == d && a == b && a != nullptr;
}
auto composite(bool c, __attribute__((opencl_generic)) int *g, __attribute__((opencl_global)) int *q) {
  return c ? g : q;
}
template <class A, class B> constexpr bool same_v = false;
template <class A> constexpr bool same_v<A, A> = true;
static_assert(same_v<decltype(composite(true, nullptr, nullptr)), __attribute__((opencl_generic)) int *>);
void refs(__attribute__((opencl_global)) int &g, __attribute__((address_space(2))) int &a) {
  __attribute__((opencl_generic)) int &r = g;
  __attribute__((opencl_generic)) const int &cr = g;
  __attribute__((address_space(2))) int &s = a;
  __attribute__((address_space(2))) const int &cs = a;
  const int &t = 3;
  __attribute__((opencl_generic)) const int &u = 3;
  (void)r; (void)cr; (void)s; (void)cs; (void)t; (void)u;
}
struct S { int m; int n[2]; };
extern __attribute__((address_space(1))) S gs;
int member() { return gs.m + gs.n[1]; }
__attribute__((address_space(1))) int *member_ptr() { return &gs.m; }
__attribute__((address_space(1))) int &member_ref() { return gs.n[0]; }
