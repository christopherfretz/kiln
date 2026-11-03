// [expr.context]/2: discarded volatile glvalues are read (`load volatile`);
// a call returning a volatile reference is not.
struct S { volatile int m; int n; };
volatile int vg;
volatile int &h();
void f(volatile int &r, volatile int *p) {
  r;
  *p;
  vg;
  (void)r;
  static_cast<void>(*p);
  (r);
  r, vg;
  h();
}
void g(bool b, volatile int &x, volatile int &y, S &s, volatile S &vs,
       volatile int S::*pm, volatile int *a) {
  s.m;
  vs.n;
  b ? x : y;
  x ?: y;
  s.*pm;
  a[1];
  for (int i = 0; i < 2; x) ++i;
}
template <class T> void t(T &v) { v; (void)v; }
template void t<volatile int>(volatile int &);
