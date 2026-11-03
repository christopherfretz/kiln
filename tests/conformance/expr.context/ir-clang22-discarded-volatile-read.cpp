// [expr.context]/2: a discarded-value expression that is a glvalue of
// volatile-qualified type and an id-expression, subscript, member access,
// indirection, pointer-to-member operation, or a comma / conditional of
// those undergoes the lvalue-to-rvalue conversion, so the object is read
// (`load volatile`). That holds for an expression statement, a for
// increment, the left operand of a comma and the operand of a cast to
// void; a call returning `volatile int&` and a volatile class object are
// not read. Compared with clang 22's IR exactly.
//@ expect: ir
//@ requires: clang>=22
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
  b ? x : 1;
  x ?: y;
  s.*pm;
  a[1];
  for (int i = 0; i < 2; x) ++i;
}
template <class T> void t(T &v) { v; (void)v; }
template void t<volatile int>(volatile int &);
