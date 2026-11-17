// The accepted side of p11-casts-away-constness.cpp: the cv-qualification
// kept or added, and the C-style cast, which may cast it away.
//@ expect: compile-ok
//@ args: -Wno-unused-value
typedef char const *ary_t[2];
typedef const int cint2[2];
void arrays(ary_t const *p, ary_t *m, cint2 *q, int (*ok)[3]) {
  static_cast<void const *>(p);
  static_cast<void *>(m);
  const void *cv = q;
  void *v = ok;
  volatile void *vv = (volatile int (*)[2])nullptr;
  (void *)p;
  (void)cv; (void)v; (void)vv;
}
struct B {};
struct D : B {};
void classes(const B *b, const B &rb, B *mb) {
  static_cast<const D *>(b);
  static_cast<const volatile D *>(b);
  static_cast<const D &>(rb);
  static_cast<D *>(mb);
  (D *)b;
  (D &)rb;
}
